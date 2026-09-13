import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname, relative, isAbsolute } from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { renderStatus } from './context-status.mjs';

const cli = fileURLToPath(new URL('./runbook-check.mjs', import.meta.url));
const at = '2026-09-13T08:49:54Z';
const write = (root, file, value) => {
  const p = resolve(root, file); mkdirSync(dirname(p), { recursive: true });
  writeFileSync(p, typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n');
};
const git = (root, ...args) => {
  const r = spawnSync('git', ['-c','user.name=Runbook Test','-c','user.email=test@example.invalid','-c','commit.gpgsign=false','-c',`core.hooksPath=${resolve(root,'no-hooks')}`, ...args], {cwd:root,encoding:'utf8',windowsHide:true});
  assert.equal(r.status, 0, `${args.join(' ')}: ${r.stderr}`); return r.stdout.trim();
};
function temp(t) {
  const parent = resolve(tmpdir());
  const root = mkdtempSync(resolve(parent, 'glagol-runbook-'));
  t.after(() => {
    // Verify the resolved deletion target belongs to this test's temporary root.
    const rel = relative(parent, resolve(root));
    assert.ok(!isAbsolute(rel) && !rel.startsWith('..') && rel.startsWith('glagol-runbook-'));
    rmSync(root, {recursive:true,force:true,maxRetries:3});
  });
  return root;
}
function fixture(t) {
  const outer = temp(t), root = resolve(outer, 'проект with spaces'); mkdirSync(root);
  write(root,'package.json',{version:'0.4.0'});
  write(root,'src-tauri/tauri.conf.json',{version:'0.4.0'});
  write(root,'src-tauri/Cargo.toml','[package]\nname = "glagol"\nversion = "0.4.0"\n');
  write(root,'src-tauri/Cargo.lock','[[package]]\nname = "glagol"\nversion = "0.4.0"\n');
  write(root,'docs/workstreams/demo/README.md','# Demo\n');
  write(root,'docs/workstreams/demo/log.md','# Проверка\n\nИсторический путь: removed/file.rs\n');
  git(root,'init','-b','main'); git(root,'add','.'); git(root,'commit','-m','initial');
  const main = git(root,'rev-parse','HEAD'); git(root,'tag','v0.1.0');
  git(root,'checkout','-b','task'); write(root,'local.txt','local change'); git(root,'add','.'); git(root,'commit','-m','local change');
  const head = git(root,'rev-parse','HEAD');
  const project = {schema_version:1,observed_at:at,active_workstream:'demo',links:['docs/workstreams/demo/README.md'],observations:[{id:'release',label:'Релиз',status:'reported',summary:'Со слов пользователя',observed_at:at,source:'docs/workstreams/demo/log.md'}],git_refs:[{id:'baseline',sha:head,scope:'local'}]};
  const check = {id:'validation',status:'PASS',method:'command',command:'fixture-test',cwd:'.',at,environment:{node:'test fixture'},exit_code:0,output:'fixture OK',evidence:'docs/workstreams/demo/log.md#проверка'};
  const state = {schema_version:1,id:'demo',title:'Тестовая серия',status:'active',next:'R2',reason:null,claim:null,links:[],steps:[{id:'R1',title:'Done',status:'done',depends_on:[],required:['validation'],checks:[check]},{id:'R2',title:'Next',status:'pending',depends_on:['R1'],required:['validation'],checks:[]}]};
  const save = (render=true) => {
    write(root,'docs/context/project.json',project); write(root,'docs/workstreams/demo/state.json',state);
    if(render) write(root,'docs/STATUS.md',renderStatus(root));
  };
  save();
  return {root,project,state,check,save,main,head};
}
function run(f, code, message) {
  const r=spawnSync(process.execPath,[cli,'--root',f.root],{encoding:'utf8',windowsHide:true});
  assert.equal(r.status,code,`${r.stdout}\n${r.stderr}`);
  if(message) assert.match(r.stdout+r.stderr,message);
  return r;
}

test('local commit outside main passes; output deterministic in Cyrillic/spaced path', t=>{
  const f=fixture(t); const one=renderStatus(f.root), two=renderStatus(f.root);
  assert.equal(one,two); run(f,0,/runbook-check OK/);
});
test('terminal done series needs null NEXT, not ARCHIVED',t=>{
  const f=fixture(t); f.state.steps.pop(); f.state.status='done'; f.state.next=null; f.project.active_workstream=null; f.save(); run(f,0);
});
test('active claim and required dependencies pass',t=>{
  const f=fixture(t); f.state.steps[1].status='in_progress'; f.state.claim={owner:'test',at,branch:'task'}; f.save(); run(f,0);
});

const mutations = [
  ['unsupported schema',(f)=>f.state.schema_version=2,/unsupported schema/],
  ['unknown state field',(f)=>f.state.typo=true,/unknown field/],
  ['missing state field',(f)=>delete f.state.claim,/missing field/],
  ['duplicate IDs',(f)=>f.state.steps[1].id='R1',/duplicate ID/],
  ['two running steps',(f)=>{f.state.steps.forEach(s=>{s.status='in_progress';s.depends_on=[];});f.state.next='R1';},/more than one in_progress/],
  ['missing NEXT',(f)=>f.state.next=null,/NEXT/],
  ['NEXT points to done',(f)=>f.state.next='R1',/NEXT/],
  ['NEXT unknown ID',(f)=>f.state.next='R9',/NEXT/],
  ['required check not run',(f)=>{f.check.status='NOT_RUN';f.check.exit_code=null;},/required check validation is not PASS/],
  ['done without validation',(f)=>f.state.steps[0].required=[],/done requires validation/],
  ['PASS with failing exit code',(f)=>f.check.exit_code=7,/exit_code contradicts/],
  ['FAIL with successful exit code',(f)=>{f.check.status='FAIL';f.check.exit_code=0;},/exit_code contradicts/],
  ['manual fabricated exit code',(f)=>f.check.method='manual',/manual check must have null/],
  ['missing environment',(f)=>f.check.environment={},/environment required/],
  ['running without claim',(f)=>f.state.steps[1].status='in_progress',/expected object/],
  ['claim without running',(f)=>f.state.claim={owner:'test',at,branch:'task'},/claim without running/],
  ['invalid claim time',(f)=>{f.state.steps[1].status='in_progress';f.state.claim={owner:'test',at:'yesterday',branch:'task'};},/invalid claim/],
  ['blocked without reason',(f)=>f.state.steps[1].status='blocked',/reason required/],
  ['paused without reason',(f)=>{f.state.status='paused';f.project.active_workstream=null;},/requires reason/],
  ['bad dependency',(f)=>f.state.steps[1].depends_on=['R9'],/invalid dependency/],
  ['cycle',(f)=>{f.state.steps[0].status='pending';f.state.steps[0].depends_on=['R2'];f.state.next='R1';},/dependency cycle/],
  ['unmet running dependency',(f)=>{f.state.steps[0].status='pending';f.state.steps[1].status='in_progress';},/dependency R1 not done/],
  ['unselected active series',(f)=>f.project.active_workstream=null,/active series without selection/],
  ['invalid selection path',(f)=>f.project.active_workstream='../escape',/invalid active_workstream/],
  ['missing evidence file',(f)=>f.check.evidence='docs/missing.md',/missing link/],
  ['missing evidence anchor',(f)=>f.check.evidence='docs/workstreams/demo/log.md#absent',/missing anchor/],
  ['absolute path',(f)=>f.project.links=[f.root.replaceAll('\\','/')],/invalid repository path/],
  ['external source',(f)=>f.project.observations[0].source='https://example.com',/invalid repository path/],
  ['unknown check status',(f)=>f.check.status='MAYBE',/invalid check status/],
  ['false main claim',(f)=>f.project.git_refs[0].scope='main',/not reachable from main/],
  ['invented commit',(f)=>f.project.git_refs[0].sha='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',/commit does not exist/],
  ['unresolved without explanation',(f)=>f.project.git_refs[0].scope='historical_unresolved',/requires reason/],
];
for(const [name,mutate,expected] of mutations) test(name,t=>{
  const f=fixture(t); mutate(f);
  // Some deliberately invalid objects cannot render; retain the last valid view.
  f.save(false); try {write(f.root,'docs/STATUS.md',renderStatus(f.root));} catch {}
  run(f,1,expected);
});
test('corrupt JSON does not hide workstream',t=>{
  const f=fixture(t);write(f.root,'docs/workstreams/demo/state.json','{oops');run(f,1,/series demo/);
});
test('missing state in an additional directory is an error',t=>{
  const f=fixture(t);write(f.root,'docs/workstreams/forgotten/README.md','# Forgotten');run(f,1,/forgotten/);
});
test('manual edit of generated status fails',t=>{
  const f=fixture(t);write(f.root,'docs/STATUS.md','looks fine');run(f,1,/STATUS.md stale/);
});
test('version drift fails',t=>{
  const f=fixture(t);write(f.root,'src-tauri/tauri.conf.json',{version:'0.3.0'});run(f,1,/differs/);
});
test('missing application version fails',t=>{
  const f=fixture(t);write(f.root,'package.json',{});run(f,1,/valid application version required/);
});
test('dotted release workstream ID passes including active selection',t=>{
  const f=fixture(t);const dotted={...f.state,id:'verification-0.4.0'};
  write(f.root,'docs/workstreams/verification-0.4.0/state.json',dotted);
  write(f.root,'docs/workstreams/verification-0.4.0/README.md','# Verification');
  write(f.root,'docs/workstreams/verification-0.4.0/log.md','# Log');
  f.state.status='paused';f.state.reason='queued';f.project.active_workstream=dotted.id;f.save();run(f,0);
});
test('artifact SHA, UUID and deleted historical paths are not Git evidence',t=>{
  const f=fixture(t);write(f.root,'docs/workstreams/demo/log.md','# Проверка\n50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437\n01234567-abcd-4321-abcd-0123456789ab\nremoved/file.rs\n');run(f,0);
});
test('CRLF JSON and Markdown accepted',t=>{
  const f=fixture(t);for(const p of ['docs/STATUS.md','docs/context/project.json','docs/workstreams/demo/state.json','docs/workstreams/demo/log.md','src-tauri/Cargo.lock']) write(f.root,p,readFileSync(resolve(f.root,p),'utf8').replaceAll('\n','\r\n'));run(f,0);
});
test('existing path outside repository rejected',t=>{
  const f=fixture(t);write(dirname(f.root),'outside.md','outside');f.project.links=['../outside.md'];f.save();run(f,1,/escapes repository/);
});
test('valid release ref and missing tag distinguished',t=>{
  const f=fixture(t);f.project.git_refs=[{id:'release',sha:f.main,scope:'release',ref:'refs/tags/v0.1.0'}];f.save();run(f,0);
  f.project.git_refs[0].ref='refs/tags/missing';f.save();run(f,2,/INCOMPLETE/);
});
test('missing main ref is incomplete',t=>{
  const f=fixture(t);git(f.root,'branch','-D','main');f.project.git_refs[0].scope='main';f.save();run(f,2,/missing ref main/);
});
test('historical unresolved requires explicit evidence and is only a note',t=>{
  const f=fixture(t);f.project.git_refs=[{id:'old',sha:'aaaaaaaa',scope:'historical_unresolved',reason:'squashed, mapping described in log',evidence:'docs/workstreams/demo/log.md'}];f.save();run(f,0,/historical_unresolved/);
});
test('shallow history is incomplete even if HEAD reference resolves',t=>{
  const f=fixture(t);git(f.root,'add','.');git(f.root,'commit','-m','fixture context');
  const clone=resolve(temp(t),'shallow');
  git(dirname(clone),'clone','--depth','1',pathToFileURL(f.root).href,clone);
  run({root:clone},2,/shallow history/);
});
test('read-only gate leaves state, status and git status unchanged',t=>{
  const f=fixture(t);const before=git(f.root,'status','--porcelain');
  const status=readFileSync(resolve(f.root,'docs/STATUS.md'),'utf8');run(f,0);
  assert.equal(git(f.root,'status','--porcelain'),before);
  assert.equal(readFileSync(resolve(f.root,'docs/STATUS.md'),'utf8'),status);
  assert.ok(existsSync(resolve(f.root,'docs/workstreams/demo/state.json')));
});
