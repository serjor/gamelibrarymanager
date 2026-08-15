import { GlobalRegistrator } from "@happy-dom/global-registrator";

// Bun has no DOM: happy-dom registers it in globalThis before each suite.
GlobalRegistrator.register();
