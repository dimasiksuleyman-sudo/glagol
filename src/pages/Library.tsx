import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

/**
 * Library page — placeholder for Sprint 2's document library.
 *
 * Sprint 1 ships only the synthesize-and-save flow; persistent storage
 * of generated audio with resume-playback lands in Sprint 2.
 */
export function Library() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-semibold tracking-tight">Библиотека</h2>
        <p className="text-muted-foreground mt-1 text-sm">
          История озвученных документов.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Скоро</CardTitle>
          <CardDescription>
            В Sprint 2 здесь появится постоянное хранение озвученных документов с
            возобновлением прослушивания.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <p className="text-muted-foreground text-sm">
            А пока используйте страницу <strong>Озвучить</strong> — сохраняйте WAV-файлы в
            любую папку через системный диалог.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
