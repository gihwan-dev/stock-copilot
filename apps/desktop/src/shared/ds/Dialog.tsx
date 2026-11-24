import type { DialogActions } from "./control-types";
import { Button } from "./Button";
import { useRef, type ReactElement } from "react";
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { focusDialogInput } from "../platform/input";
import { text, type UiText } from "../lib";
import { Icon } from "./Icon";
import { ScrollArea } from "./ScrollArea";
type CloseReason = "escape" | "outside" | "close-button";
type Props = {
  readonly open: "open" | "closed";
  readonly title: UiText;
  readonly onClose?: (reason: CloseReason) => void;
  readonly children: ReactElement | readonly ReactElement[];
  readonly actions?: DialogActions;
  readonly wide?: "normal" | "wide";
};
const widths = {
  normal:
    "w-full max-w-dialog-width max-h-full scroll-flex flex flex-col bg-surface-raised rounded-lg shadow-popover",
  wide: "w-full max-w-dialog-width-wide max-h-full scroll-flex flex flex-col bg-surface-raised rounded-lg shadow-popover",
} as const;
export const Dialog = (props: Props): ReactElement => {
  const wide = props.wide ?? "normal";
  const popupRef = useRef<HTMLDivElement>(null);
  return (
    <BaseDialog.Root<void>
      open={props.open === "open"}
      onOpenChange={(open, details): void => {
        if (open || props.open === "closed" || props.onClose === undefined)
          return;
        if (details.reason === "escape-key") props.onClose("escape");
        if (details.reason === "outside-press") props.onClose("outside");
        if (details.reason === "close-press") props.onClose("close-button");
      }}
    >
      <BaseDialog.Portal>
        <BaseDialog.Backdrop
          data-dialog-backdrop=""
          className="dialog-overlay z-modal bg-overlay"
        />
        <BaseDialog.Viewport className="dialog-overlay z-modal flex items-center justify-center p-space-6">
          <BaseDialog.Popup
            aria-modal="true"
            ref={popupRef}
            initialFocus={(): void => {
              focusDialogInput(popupRef);
            }}
            className={widths[wide]}
          >
            <header className="flex items-center gap-space-2 min-h-toolbar-height py-space-2 px-space-5 border-b-hairline border-solid border-line flex-none">
              <BaseDialog.Title className="flex-1 status-sentence text-title-md text-ink">
                {props.title}
              </BaseDialog.Title>
              {props.onClose === undefined ? null : (
                <BaseDialog.Close
                  aria-label={text("ds.close", "ko")}
                  className="flex-none inline-grid place-items-center size-control-height rounded-md bg-transparent text-ink-muted cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
                >
                  <Icon name="x" />
                </BaseDialog.Close>
              )}
            </header>
            <ScrollArea direction="both">
              <div className="p-space-5 flex flex-col gap-space-4">
                {props.children}
              </div>
            </ScrollArea>
            <DialogFooter actions={props.actions} />
          </BaseDialog.Popup>
        </BaseDialog.Viewport>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
};

const DialogFooter = (props: {
  readonly actions: DialogActions | undefined;
}): ReactElement | null =>
  props.actions === undefined ||
  (props.actions.cancel === undefined &&
    props.actions.confirm === undefined) ? null : (
    <footer className="flex flex-wrap justify-end gap-space-2 px-space-5 py-space-3 border-t-hairline border-solid border-line flex-none">
      {props.actions.cancel === undefined ? null : (
        <Button
          variant="secondary"
          onClick={props.actions.cancel.onPress}
          disabled={props.actions.cancel.disabled ?? "enabled"}
        >
          {props.actions.cancel.label}
        </Button>
      )}
      {props.actions.confirm === undefined ? null : (
        <Button
          variant="primary"
          onClick={props.actions.confirm.onPress}
          disabled={props.actions.confirm.disabled ?? "enabled"}
        >
          {props.actions.confirm.label}
        </Button>
      )}
    </footer>
  );
