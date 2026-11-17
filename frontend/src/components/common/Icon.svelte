<script lang="ts">
  import { GLYPHS, STROKE, type Glyph, type GlyphName } from "$lib/icons";

  interface Props {
    name: GlyphName;
    label?: string;
    class?: string;
  }

  let { name, label, class: extra = "" }: Props = $props();

  const glyph: Glyph = $derived(GLYPHS[name]);
</script>

<svg
  class="icon {extra}"
  viewBox="0 0 {glyph.grid} {glyph.grid}"
  fill="none"
  stroke="currentColor"
  stroke-width={STROKE[glyph.grid]}
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
  ><path d={glyph.d} />{#if glyph.line}<path d={glyph.line} />{/if}</svg
>

<style>
  .icon {
    flex: none;
    width: var(--icon-size);
    height: var(--icon-size);
    vertical-align: -0.125em;
  }
</style>
