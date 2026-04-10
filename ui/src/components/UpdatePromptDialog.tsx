import React from "react";
import type { ChangelogSection } from "@/types/api";
import { Button, Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui";

export function UpdatePromptDialog(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  overviewTitle: string;
  loadingText: string;
  loadFailText: string;
  sections: ChangelogSection[] | null;
  loading: boolean;
  loadError: string | null;
  updateText: string;
  laterText: string;
  ignoreText: string;
  onUpdate: () => void;
  onLater: () => void;
  onIgnore: () => void;
  busy?: boolean;
}) {
  const { sections, loading, loadError } = props;
  const busy = props.busy ?? false;

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{props.title}</DialogTitle>
          <DialogDescription>{props.description}</DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <div className="text-sm font-medium">{props.overviewTitle}</div>
          <div className="max-h-[320px] overflow-auto rounded-md border bg-muted/10 px-3 py-2">
            {loading ? (
              <div className="text-xs text-muted-foreground">{props.loadingText}</div>
            ) : loadError ? (
              <div className="text-xs text-muted-foreground">{props.loadFailText}</div>
            ) : sections && sections.length > 0 ? (
              <div className="space-y-3">
                {sections.map((sec) => (
                  <div key={sec.title}>
                    <div className="text-xs font-semibold">{sec.title}</div>
                    {sec.items.length > 0 ? (
                      <ul className="mt-1 list-disc pl-4 text-xs text-muted-foreground space-y-0.5">
                        {sec.items.map((it) => (
                          <li key={it}>{it}</li>
                        ))}
                      </ul>
                    ) : (
                      <div className="text-xs text-muted-foreground">-</div>
                    )}
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-xs text-muted-foreground">-</div>
            )}
          </div>
        </div>

        <DialogFooter className="flex w-full items-center justify-between gap-2 sm:gap-0">
          <Button variant="destructive" onClick={props.onIgnore} disabled={busy}>
            {props.ignoreText}
          </Button>
          <div className="flex items-center gap-2">
            <Button onClick={props.onUpdate} disabled={busy}>
              {props.updateText}
            </Button>
            <Button variant="outline" onClick={props.onLater} disabled={busy}>
              {props.laterText}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
