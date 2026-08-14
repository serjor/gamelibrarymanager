import { describe, expect, it, mock } from "bun:test";
import { render, screen } from "@testing-library/react";

const invoke = mock(async () => ({ version: "0.1.0", stores: ["steam"] }));
mock.module("@tauri-apps/api/core", () => ({ invoke }));

const { App } = await import("./App");

describe("App", () => {
  it("muestra la información que devuelve el backend", async () => {
    render(<App />);
    expect(await screen.findByText(/v0\.1\.0/)).toBeDefined();
    expect(invoke).toHaveBeenCalledWith("app_info");
  });

  it("muestra el error en lugar de quedarse en blanco si el backend falla", async () => {
    invoke.mockImplementationOnce(() => Promise.reject(new Error("sin backend")));
    render(<App />);
    expect(await screen.findByRole("alert")).toBeDefined();
  });
});
