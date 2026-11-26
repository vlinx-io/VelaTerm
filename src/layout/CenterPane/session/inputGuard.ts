import { installWebkitArrowInputGuard } from "../../../platform/webkitArrowInputGuard";

/**
 * Delegates to the shared WKWebView arrow-control guard on a subtree root so panes and isolated
 * component tests guard dynamically mounted editors and clean up on unmount.
 */
export function attachChatInputGuard(root: HTMLDivElement | null) {
  if (!root) return;
  return installWebkitArrowInputGuard(root);
}
