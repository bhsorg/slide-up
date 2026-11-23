import { ask } from "@tauri-apps/plugin-dialog";

export interface ConfirmOptions {
  title?: string;
  kind?: "info" | "warning" | "error";
  okLabel?: string;
  cancelLabel?: string;
}

/**
 * Shows a native system confirmation dialog (or falls back to window.confirm in browser).
 * Returns true if the user confirmed, false otherwise.
 */
export async function confirmAction(
  message: string,
  options: ConfirmOptions = {},
): Promise<boolean> {
  try {
    return await ask(message, {
      title: options.title ?? "Slide Up Confirmation",
      kind: options.kind ?? "warning",
      okLabel: options.okLabel ?? "Confirm",
      cancelLabel: options.cancelLabel ?? "Cancel",
    });
  } catch (err) {
    console.warn("Native dialog failed, falling back to window.confirm:", err);
    return window.confirm(message);
  }
}
