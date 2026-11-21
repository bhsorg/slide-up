import type { Component } from "vue";

export interface ContextMenuItem {
  label: string;
  icon?: Component;
  danger?: boolean;
  disabled?: boolean;
  /** Renders a non-interactive divider instead of a menu button. */
  separator?: boolean;
}
