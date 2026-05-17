import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

/**
 * Settings page — manage the SaluteSpeech Authorization Key.
 *
 * Phase 2 skeleton: form layout only. The three buttons (Save / Test /
 * Delete) are wired in Phase 3 to call the Tauri commands and update
 * the {@link CredentialsContext} on success.
 */
export function Settings() {
  const [authKey, setAuthKey] = useState<string>("");
  // Phase 3: replace placeholder with a proper isLoading flag toggled
  // around each invoke call so the three buttons disable correctly.
  const isLoading = false;

  // Placeholder handlers — Phase 3 wires them to setCredentials() etc.
  function handleSave() {
    // Phase 3.
  }
  function handleTest() {
    // Phase 3.
  }
  function handleDelete() {
    // Phase 3.
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-semibold tracking-tight">Настройки</h2>
        <p className="text-muted-foreground mt-1 text-sm">
          Authorization Key из консоли разработчика Сбера. Хранится в Windows Credential
          Manager.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>SaluteSpeech Authorization Key</CardTitle>
          <CardDescription>
            Получите ключ на developers.sber.ru, проект SaluteSpeech API → скопируйте
            готовую Base64-строку.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="auth-key">Ключ</Label>
            <Input
              id="auth-key"
              type="password"
              autoComplete="off"
              spellCheck={false}
              placeholder="Base64(client_id:client_secret)"
              value={authKey}
              onChange={(event) => setAuthKey(event.target.value)}
              disabled={isLoading}
            />
          </div>

          <div className="flex flex-wrap gap-2">
            <Button onClick={handleSave} disabled={isLoading || authKey.trim().length === 0}>
              Сохранить
            </Button>
            <Button variant="secondary" onClick={handleTest} disabled={isLoading}>
              Проверить
            </Button>
            <Button variant="destructive" onClick={handleDelete} disabled={isLoading}>
              Удалить
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
