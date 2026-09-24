import { Navigate, Route, Routes } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { Dictation } from "@/pages/Dictation";
import { Library } from "@/pages/Library";
import { Settings } from "@/pages/Settings";
import { Synthesize } from "@/pages/Synthesize";
import { usePreferences } from "@/contexts/PreferencesContext";
import { Onboarding } from "@/components/Onboarding";

/**
 * Top-level route table. Every page sits beneath the {@link AppShell}
 * layout (sidebar + Outlet); `/` redirects to `/synthesize` as the
 * default landing page.
 *
 * `<BrowserRouter>` lives in {@link ./main.tsx} so the TTS readiness
 * context provider can sit outside (and survive route changes).
 */
function App() {
  const { preferences } = usePreferences();
  if (!preferences || preferences.onboarding_stage !== "complete") return <Onboarding />;
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<Navigate to="/synthesize" replace />} />
        <Route path="/synthesize" element={<Synthesize />} />
        <Route path="/library" element={<Library />} />
        <Route path="/dictation" element={<Dictation />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="*" element={<Navigate to="/synthesize" replace />} />
      </Route>
    </Routes>
  );
}

export default App;
