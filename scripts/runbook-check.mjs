import { existsSync, readFileSync, readdirSync, realpathSync, statSync } from 'node:fs';
import { resolve, relative, isAbsolute } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { ROOT, statusMatches } from './context-status.mjs';
import { checkVersions } from './check-version.mjs';

const idPattern = /^[a-zA-Z0-9]+(?:-[a-zA-Z0-9]+)*$/;
const seriesPattern = /^[a-z0-9]+(?:[.-][a-z0-9]+)*$/;
const terminal = (s) => ['done', 'cancelled'].includes(s);
const need = (condition, message) => { if (!condition) throw new Error(message); };
const text = (v) => typeof v === 'string' && v.trim().length > 0;
const date = (v) => typeof v === 'string' && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(v) && Number.isFinite(Date.parse(v));
function record(v, required, optional = []) {
  need(v && typeof v === 'object' && !Array.isArray(v), 'expected object');
  for (const key of required) need(Object.hasOwn(v, key), `missing field: ${key}`);
  for (const key of Object.keys(v)) need([...required, ...optional].includes(key), `unknown field: ${key}`);
}
function strings(v, name) {
  need(Array.isArray(v) && v.every(text), `${name}: expected string array`);
  need(new Set(v).size === v.length, `${name}: duplicate values`);
}
function uniqueIds(v, name) {
  need(Array.isArray(v), `${name}: expected array`);
  const ids = v.map((item) => item?.id);
  need(ids.every((id) => typeof id === 'string' && idPattern.test(id)), `${name}: invalid ID`);
  need(new Set(ids).size === ids.length, `${name}: duplicate ID`);
}

/** Read-only integrity gate. Code 2 means missing Git evidence, never success. */
export function checkRunbooks(root = ROOT) {
  root = resolve(root);
  const failures = [], incomplete = [], notes = [];
  const attempt = (label, fn) => { try { return fn(); } catch (e) { failures.push(`${label}: ${e.message}`); return null; } };
  const read = (p) => JSON.parse(readFileSync(resolve(root, p), 'utf8'));
  const outside = (p) => { const r = relative(realpathSync(root), p); return r === '..' || r.startsWith('../') || r.startsWith('..\\') || isAbsolute(r); };
  function link(p, directory = false) {
    need(text(p) && !p.includes('\\') && !isAbsolute(p) && !/^[a-zA-Z][\w+.-]*:/.test(p), `invalid repository path: ${p}`);
    const [file, anchor, ...extra] = p.split('#');
    need(file && extra.length === 0 && (anchor === undefined || anchor), `invalid link: ${p}`);
    const target = resolve(root, file);
    need(existsSync(target), `missing link: ${p}`);
    need(!outside(realpathSync(target)), `path escapes repository: ${p}`);
    need(directory ? statSync(target).isDirectory() : statSync(target).isFile(), `wrong path type: ${p}`);
    if (anchor !== undefined) {
      need(!directory, `directory cannot have anchor: ${p}`);
      const body = readFileSync(target, 'utf8');
      const headings = [...body.matchAll(/^#{1,6}\s+(.+)\r?$/gm)].map((m) => m[1].trim().toLowerCase().replace(/[^\p{L}\p{N}_\-\s]/gu, '').replace(/\s/g, '-'));
      const explicit = [...body.matchAll(/\bid=["']([^"']+)["']/g)].map((m) => m[1]);
      need([...headings, ...explicit].includes(anchor), `missing anchor: ${p}`);
    }
  }
  function links(v) { strings(v, 'links'); v.forEach((p) => link(p)); }
  function checkEvidence(c) {
    record(c, ['id','status','method','command','cwd','at','environment','exit_code','output','evidence']);
    need(['PASS','FAIL','NOT_RUN','BLOCKED'].includes(c.status), `${c.id}: invalid check status`);
    need(['command','manual'].includes(c.method), `${c.id}: invalid method`);
    need(text(c.command) && text(c.output) && date(c.at), `${c.id}: command/output/UTC date required`);
    need(c.environment && typeof c.environment === 'object' && !Array.isArray(c.environment) && Object.keys(c.environment).length && Object.values(c.environment).every(text), `${c.id}: environment required`);
    link(c.cwd, true); link(c.evidence);
    if (c.method === 'command' && ['PASS','FAIL'].includes(c.status)) {
      need(Number.isInteger(c.exit_code), `${c.id}: command exit_code required`);
      need(c.status === 'PASS' ? c.exit_code === 0 : c.exit_code !== 0, `${c.id}: exit_code contradicts ${c.status}`);
    } else need(c.exit_code === null, `${c.id}: unexecuted/manual check must have null exit_code`);
  }
  function checkSeries(s, folder) {
    record(s, ['schema_version','id','title','status','next','reason','claim','links','steps']);
    need(s.schema_version === 1, 'unsupported schema_version');
    need(s.id === folder && seriesPattern.test(s.id) && text(s.title), 'invalid series ID/title');
    need(['active','paused','done','cancelled'].includes(s.status), 'invalid series status');
    need(s.reason === null || text(s.reason), 'invalid reason');
    if (['paused','cancelled'].includes(s.status)) need(text(s.reason), 'paused/cancelled series requires reason');
    links(s.links);
    for (const name of ['README.md','log.md']) link(`docs/workstreams/${folder}/${name}`);
    uniqueIds(s.steps, 'steps'); need(s.steps.length > 0, 'steps cannot be empty');
    const byId = new Map(s.steps.map((step) => [step.id, step]));
    for (const step of s.steps) {
      record(step, ['id','title','status','depends_on','required','checks'], ['reason']);
      need(text(step.title), `${step.id}: title required`);
      need(['pending','in_progress','blocked','done','cancelled'].includes(step.status), `${step.id}: invalid step status`);
      if (['blocked','cancelled'].includes(step.status)) need(text(step.reason), `${step.id}: reason required`);
      strings(step.depends_on, 'depends_on'); strings(step.required, 'required');
      for (const dep of step.depends_on) {
        need(dep !== step.id && byId.has(dep), `${step.id}: invalid dependency ${dep}`);
        if (['in_progress','done'].includes(step.status)) need(byId.get(dep).status === 'done', `${step.id}: dependency ${dep} not done`);
      }
      uniqueIds(step.checks, `${step.id} checks`); step.checks.forEach(checkEvidence);
      if (step.status === 'done') {
        need(step.required.length > 0, `${step.id}: done requires validation`);
        for (const required of step.required) need(step.checks.some((c) => c.id === required && c.status === 'PASS'), `${step.id}: required check ${required} is not PASS`);
      }
    }
    const visited = new Set(), visiting = new Set();
    function visit(id) {
      need(!visiting.has(id), `dependency cycle at ${id}`);
      if (visited.has(id)) return;
      visiting.add(id); byId.get(id).depends_on.forEach(visit); visiting.delete(id); visited.add(id);
    }
    s.steps.forEach((step) => visit(step.id));
    const unfinished = s.steps.filter((step) => !terminal(step.status));
    const running = s.steps.filter((step) => step.status === 'in_progress');
    need(running.length <= 1, 'more than one in_progress step');
    if (terminal(s.status)) {
      need(s.next === null && unfinished.length === 0, 'terminal series must have next=null and no unfinished steps');
      if (s.status === 'done') need(s.steps.every((step) => step.status === 'done'), 'done series has cancelled steps');
    } else need(unfinished.length > 0 && s.next === unfinished[0].id, 'NEXT must be first unfinished step');
    if (running.length) {
      need(s.status === 'active' && running[0].id === s.next, 'running step must be active NEXT');
      record(s.claim, ['owner','at','branch']);
      need(text(s.claim.owner) && text(s.claim.branch) && date(s.claim.at), 'invalid claim');
      if (Date.now() - Date.parse(s.claim.at) > 86400000) notes.push(`${s.id}: claim older than 24h; verify before taking over`);
    } else need(s.claim === null, 'claim without running step');
    return s;
  }

  const project = attempt('project', () => {
    const p = read('docs/context/project.json');
    record(p, ['schema_version','observed_at','active_workstream','links','observations','git_refs']);
    need(p.schema_version === 1, 'unsupported schema_version');
    need(date(p.observed_at), 'observed_at must be UTC ISO');
    need(p.active_workstream === null || (typeof p.active_workstream === 'string' && seriesPattern.test(p.active_workstream)), 'invalid active_workstream');
    links(p.links); uniqueIds(p.observations, 'observations'); need(p.observations.length > 0, 'observations empty');
    for (const o of p.observations) {
      record(o, ['id','label','status','summary','observed_at','source']);
      need(text(o.label) && text(o.summary) && date(o.observed_at), `${o.id}: invalid observation`);
      need(['observed','reported','unknown'].includes(o.status), `${o.id}: invalid observation status`); link(o.source);
    }
    uniqueIds(p.git_refs, 'git_refs');
    return p;
  });
  const series = [];
  attempt('workstreams', () => {
    const entries = readdirSync(resolve(root, 'docs/workstreams'), { withFileTypes: true });
    const dirs = entries.filter((e) => e.isDirectory() || e.isSymbolicLink());
    need(dirs.length > 0, 'no workstreams found');
    for (const entry of dirs) {
      const s = attempt(`series ${entry.name}`, () => {
        link(`docs/workstreams/${entry.name}/state.json`);
        return checkSeries(read(`docs/workstreams/${entry.name}/state.json`), entry.name);
      });
      if (s) series.push(s);
    }
  });
  if (project) attempt('selection', () => {
    const active = series.filter((s) => s.status === 'active');
    if (project.active_workstream === null) need(active.length === 0, 'active series without selection');
    else need(active.length === 1 && active[0].id === project.active_workstream, 'selection must name the only active series');
  });
  attempt('versions', () => checkVersions(root));

  const git = (args) => spawnSync('git', args, { cwd: root, encoding: 'utf8', windowsHide: true });
  const shallowResult = git(['rev-parse','--is-shallow-repository']);
  const gitAvailable = shallowResult.status === 0;
  const shallow = shallowResult.stdout?.trim() === 'true';
  if (!gitAvailable) incomplete.push('Git repository unavailable; cannot check commit evidence');
  else if (shallow) incomplete.push('shallow history; full Git evidence required');
  for (const g of project?.git_refs ?? []) attempt(`git ${g.id}`, () => {
    record(g, ['id','sha','scope'], ['ref','reason','evidence']);
    need(typeof g.sha === 'string' && /^[a-f0-9]{7,40}$/i.test(g.sha), 'invalid SHA');
    need(['local','main','release','historical_unresolved'].includes(g.scope), 'invalid Git scope');
    if (g.scope === 'historical_unresolved') {
      need(text(g.reason), 'historical reference requires reason'); link(g.evidence);
      notes.push(`${g.id}: historical_unresolved (not current evidence)`); return;
    }
    if (g.scope === 'release') need(typeof g.ref === 'string' && /^refs\/tags\/[\w./-]+$/.test(g.ref) && !g.ref.includes('..'), 'release requires tag ref');
    else need(g.ref === undefined, 'ref only allowed for release');
    if (!gitAvailable) return;
    if (git(['cat-file','-e',`${g.sha}^{commit}`]).status !== 0) {
      if (shallow) incomplete.push(`${g.id}: commit not present in shallow history`);
      else throw new Error(`commit does not exist: ${g.sha}`);
      return;
    }
    let ref = 'HEAD';
    if (g.scope === 'release') ref = g.ref;
    if (g.scope === 'main') {
      ref = git(['rev-parse','--verify','origin/main^{commit}']).status === 0 ? 'origin/main' : 'main';
      notes.push(`${g.id}: reachability against locally available ${ref}; no remote freshness claim`);
    }
    if (git(['rev-parse','--verify',`${ref}^{commit}`]).status !== 0) { incomplete.push(`${g.id}: missing ref ${ref}`); return; }
    const result = git(['merge-base','--is-ancestor',g.sha,ref]);
    if (result.status !== 0) {
      if (shallow || result.status !== 1) incomplete.push(`${g.id}: ancestry cannot be established`);
      else throw new Error(`commit not reachable from ${ref}: ${g.sha}`);
    }
  });
  attempt('status', () => need(statusMatches(root), 'docs/STATUS.md stale; run pnpm context:refresh'));
  const code = failures.length ? 1 : incomplete.length ? 2 : 0;
  return { code, failures, incomplete, notes, series: series.length };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.length && !(args.length === 2 && args[0] === '--root')) {
    console.error('Usage: node scripts/runbook-check.mjs [--root path]'); process.exitCode = 1;
  } else {
    const result = checkRunbooks(args.length ? args[1] : ROOT);
    for (const line of result.notes) console.log(`note: ${line}`);
    for (const line of [...result.failures, ...result.incomplete]) console.error(line);
    console.log(`runbook-check ${result.code === 0 ? 'OK' : result.code === 2 ? 'INCOMPLETE' : 'FAILED'} (${result.series} series)`);
    process.exitCode = result.code;
  }
}
