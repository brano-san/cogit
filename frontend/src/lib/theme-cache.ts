/** `localStorage` key of the last theme: read before the first paint, because the settings
    file is read asynchronously. Its own file so vite.config.ts can import it. */
export const THEME_CACHE_KEY = "cogit.theme";
