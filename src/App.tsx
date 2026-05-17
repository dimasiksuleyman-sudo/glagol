import { Navigate, Route, Routes } from "react-router-dom";

import { AppShell } from "@/components/layout/AppShell";
import { Library } from "@/pages/Library";
import { Settings } from "@/pages/Settings";
import { Synthesize } from "@/pages/Synthesize";

/**
 * Top-level route table. Every page sits beneath the {@link AppShell}
 * layout (sidebar + Outlet); `/` redirects to `/synthesize` as the
 * default landing page.
 *
 * `<BrowserRouter>` lives in {@link ./main.tsx} so the credentials
 * context provider can sit outside (and survive route changes).
 */
function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route path="/" element={<Navigate to="/synthesize" replace />} />
        <Route path="/synthesize" element={<Synthesize />} />
        <Route path="/library" element={<Library />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="*" element={<Navigate to="/synthesize" replace />} />
      </Route>
    </Routes>
  );
}

export default App;
