import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

interface ScannedPdfDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * Alert shown when the PDF parser succeeds but extracts no text —
 * the typical signal of a scanned image-only document.
 *
 * The disclaimer is intentionally generic: it tells the user that an
 * OCR step is needed but does NOT recommend a specific service.
 * Online OCR offerings come and go on a yearly basis; pinning a name
 * or URL here would rot the disclaimer over time and route real user
 * complaints back to us instead of the failed service.
 */
export function ScannedPdfDialog({ open, onOpenChange }: ScannedPdfDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Похоже, это сканированный PDF</AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="space-y-3 text-sm">
              <p>
                Извлечь текст напрямую не получилось — PDF состоит из изображений
                страниц, а не текстового содержимого.
              </p>
              <p>
                Чтобы озвучить такой документ, его сначала нужно распознать (OCR —
                оптическое распознавание символов). В интернете есть бесплатные
                онлайн-сервисы для этого.
              </p>
              <p>
                После распознавания сохраните результат как <code>.txt</code> или{" "}
                <code>.docx</code> и попробуйте снова.
              </p>
            </div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogAction>Понятно</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
