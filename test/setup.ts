import { GlobalRegistrator } from "@happy-dom/global-registrator";

// Bun no trae DOM: happy-dom lo registra en globalThis antes de cada suite.
GlobalRegistrator.register();
