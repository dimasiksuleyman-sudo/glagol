import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
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
  useI18n();
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("This appears to be a scanned PDF")}</AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="space-y-3 text-sm">
              <p>
                {t("Text could not be extracted because this PDF contains page images rather than text.")}{" "}</p>
              <p>
                {t("To read this document aloud, first extract its text using OCR (optical character recognition). Free online OCR services are available.")}{" "}</p>
              <p>
                {t("After OCR, save the result as")}{" "}<code>.txt</code> {t("or")}{" "}
                <code>.docx</code> {t("and try again.")}{" "}</p>
            </div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogAction>{t("Got it")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
