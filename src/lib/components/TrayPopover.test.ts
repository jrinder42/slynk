import { render, screen } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
// @ts-expect-error - Component might not exist yet
import TrayPopover from "./TrayPopover.svelte";

describe("TrayPopover", () => {
  it("renders correctly with title and status", () => {
    render(TrayPopover);
    expect(screen.getByText("Slynk")).toBeInTheDocument();
    expect(screen.getByTestId("status")).toBeInTheDocument();
  });

  it("has a minimalist layout with proper classes", () => {
    const { container } = render(TrayPopover);
    const popover = container.querySelector(".popover");
    expect(popover).toBeInTheDocument();
    expect(popover).toHaveClass("minimalist");
  });
});
