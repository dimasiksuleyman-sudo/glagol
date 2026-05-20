import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

import { BackupSection } from "@/components/settings/BackupSection";
import { useCredentials } from "@/contexts/CredentialsContext";
import { deleteCredentials, setCredentials, testCredentials } from "@/lib/tauri";

/**
 * Settings page — manage the SaluteSpeech Authorization Key.
 *
 * Three independent actions, each guarded by a single `isLoading`
 * flag so two requests can't fly at once. The credentials context
 * mirrors the outcome so the Synthesize page reflects the latest
 * state without an additional probe.
 */
export function Settings() {
  const { state, setState } = useCredentials();
  const [authKey, setAuthKey] = useState<string>("");
  const [isLoading, setIsLoading] = useState<boolean>(false);

  async function handleSave() {
    setIsLoading(true);
    try {
      await setCredentials(authKey);
      toast.success("Ключ сохранён. Нажмите «Проверить», чтобы убедиться.");
      // The backend reset its cached SaluteAuth — we don't know if the
      // new key works yet, so reset the context to "unknown" until the
      // user runs Test (or until they navigate elsewhere and the mount
      // probe re-runs).
      setState("unknown");
      setAuthKey("");
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsLoading(false);
    }
  }

  async function handleTest() {
    setIsLoading(true);
    try {
      await testCredentials(true);
      toast.success("Ключ работает.");
      setState("valid");
    } catch (err) {
      toast.error(stringifyError(err));
      setState("invalid");
    } finally {
      setIsLoading(false);
    }
  }

  async function handleDelete() {
    setIsLoading(true);
    try {
      await deleteCredentials();
      toast.success("Ключ удалён.");
      setState("invalid");
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsLoading(false);
    }
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
            <p className="text-muted-foreground text-xs">
              Текущий статус: <StatusLabel state={state} />
            </p>
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

      <BackupSection />
    </div>
  );
}

function StatusLabel({ state }: { state: ReturnType<typeof useCredentials>["state"] }) {
  switch (state) {
    case "unknown":
      return <span className="text-muted-foreground">проверяем…</span>;
    case "valid":
      return <span className="text-foreground font-medium">подтверждён Сбером</span>;
    case "invalid":
      return <span className="text-muted-foreground">не настроен или не работает</span>;
  }
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
