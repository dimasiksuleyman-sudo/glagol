import { Navigate, Route, Routes } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { Dictation } from "@/pages/Dictation";
import { Library } from "@/pages/Library";
import { Settings } from "@/pages/Settings";
import { Help } from "@/pages/Help";
import { Synthesize } from "@/pages/Synthesize";
import { usePreferences } from "@/contexts/PreferencesContext";
import { Onboarding } from "@/components/Onboarding";

/**
 * Top-level route table. Every page sits beneath the {@link AppShell}
 * layout (sidebar + Outlet); `/` redirects to `/synthesize` as the
 * default landing page, or to `/dictation` when text to speech is off.
 *
 * `<BrowserRouter>` lives in {@link ./main.tsx} so the TTS readiness
 * context provider can sit outside (and survive route changes).
 */
function App() {
  const { preferences } = usePreferences();
  if (!preferences || preferences.onboarding_stage !== "complete") return <Onboarding />;
  // With text to speech off the Synthesize page (and its warm-up) never mounts.
  const ttsEnabled = preferences.tts_enabled;
  const home = ttsEnabled ? "/synthesize" : "/dictation";
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<Navigate to={home} replace />} />
        <Route path="/synthesize" element={ttsEnabled ? <Synthesize /> : <Navigate to="/dictation" replace />} />
        <Route path="/library" element={<Library />} />
        <Route path="/dictation" element={<Dictation />} />
        <Route path="/settings" element={<Navigate to="/settings/dictation" replace />} />
        <Route path="/settings/:tab" element={<Settings />} />
        <Route path="/help" element={<Help />} />
        <Route path="*" element={<Navigate to={home} replace />} />
      </Route>
    </Routes>
  );
}

export default App;
