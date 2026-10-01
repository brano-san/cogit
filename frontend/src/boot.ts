import "./app.css";
import { bootTheme } from "$stores/theme.svelte";

// Imported first by every window's entry: the tokens are in place before anything mounts.
bootTheme();
