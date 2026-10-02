<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  /** The app's push button: the one look `Find`, `Stage lines`, dialog footers and the
      metadata rows share. `sm` fits a panel bar or a metadata row (`--h-button-sm`), `md` a
      dialog footer (`--h-button`). The look is the global `.btn` in app.css, which dialogs
      and panel bars also write as a class. `title` / `data-tip` are answered by the shared
      tooltip. */
  interface Props extends Omit<HTMLButtonAttributes, "class" | "children"> {
    variant?: "default" | "primary" | "danger";
    size?: "sm" | "md";
    class?: string;
    children?: Snippet;
  }

  let { variant = "default", size = "sm", class: cls = "", type = "button", children, ...rest }: Props = $props();
</script>

<button {type} class="btn {size} {variant} {cls}" {...rest}>{@render children?.()}</button>
