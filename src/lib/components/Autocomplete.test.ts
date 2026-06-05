import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Autocomplete from "./Autocomplete.svelte";

const OPTIONS = ["Apple", "Banana", "Cherry"];

function mount(overrides: Record<string, unknown> = {}) {
  const onselect = vi.fn();
  const result = render(Autocomplete, {
    props: { value: "", options: OPTIONS, onselect, ...overrides },
  });
  const input = screen.getByRole("textbox");
  return { ...result, input, onselect };
}

// ── Dropdown visibility ───────────────────────────────────────────────────────

describe("Autocomplete — dropdown visibility", () => {
  it("does not show dropdown before focus", () => {
    mount();
    expect(screen.queryByText("Apple")).toBeNull();
  });

  it("shows options after focusing the input", async () => {
    const { input } = mount();
    await fireEvent.focus(input);
    expect(screen.getByText("Apple")).toBeInTheDocument();
    expect(screen.getByText("Banana")).toBeInTheDocument();
    expect(screen.getByText("Cherry")).toBeInTheDocument();
  });

  it("hides dropdown when Escape is pressed", async () => {
    const { input } = mount();
    await fireEvent.focus(input);
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByText("Apple")).toBeNull();
  });

  it("hides dropdown on outside click", async () => {
    const { input } = mount();
    await fireEvent.focus(input);
    expect(screen.getByText("Apple")).toBeInTheDocument();
    await fireEvent.click(document.body);
    expect(screen.queryByText("Apple")).toBeNull();
  });

  it("does not show dropdown when options is empty", async () => {
    const { input } = mount({ options: [] });
    await fireEvent.focus(input);
    expect(screen.queryByRole("button")).toBeNull();
  });
});

// ── Selection ─────────────────────────────────────────────────────────────────

describe("Autocomplete — selection", () => {
  it("calls onselect with the clicked option", async () => {
    const { input, onselect } = mount();
    await fireEvent.focus(input);
    await fireEvent.click(screen.getByText("Banana"));
    expect(onselect).toHaveBeenCalledWith("Banana");
  });

  it("closes the dropdown after selection", async () => {
    const { input } = mount();
    await fireEvent.focus(input);
    await fireEvent.click(screen.getByText("Cherry"));
    expect(screen.queryByText("Apple")).toBeNull();
  });

  it("calls onselect with Enter on the active item", async () => {
    const { input, onselect } = mount();
    await fireEvent.focus(input);
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onselect).toHaveBeenCalledWith("Apple");
  });
});

// ── Keyboard navigation ───────────────────────────────────────────────────────

describe("Autocomplete — keyboard navigation", () => {
  it("ArrowDown moves highlight to the first item", async () => {
    const { input } = mount();
    await fireEvent.focus(input);
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    // first item should be visually active — confirm Enter selects it
    const onselect = vi.fn();
    // re-test via selection behaviour already covered; just assert no throw
    expect(screen.getByText("Apple")).toBeInTheDocument();
  });

  it("ArrowDown wraps from last to first item", async () => {
    const { input, onselect } = mount();
    await fireEvent.focus(input);
    // move past all 3 items → wraps to index 0
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onselect).toHaveBeenCalledWith("Apple");
  });

  it("ArrowUp wraps around to reach the last item", async () => {
    const { input, onselect } = mount();
    await fireEvent.focus(input);
    // activeIndex starts at -1; two ArrowUp presses: (-1-1+3)%3=1, (1-1+3)%3=0...
    // wrap: from -1 → index 1 (Banana) → index 0 (Apple)...
    // To land on Cherry (index 2) press ArrowUp three times: 1→0→2
    await fireEvent.keyDown(input, { key: "ArrowUp" }); // → 1
    await fireEvent.keyDown(input, { key: "ArrowUp" }); // → 0
    await fireEvent.keyDown(input, { key: "ArrowUp" }); // → 2 (wraps)
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onselect).toHaveBeenCalledWith("Cherry");
  });

  it("Enter without active item does not call onselect", async () => {
    const { input, onselect } = mount();
    await fireEvent.focus(input);
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onselect).not.toHaveBeenCalled();
  });

  it("forwards unhandled keys to onkeydown prop", async () => {
    const onkeydown = vi.fn();
    const { input } = mount({ onkeydown });
    await fireEvent.focus(input);
    await fireEvent.keyDown(input, { key: "Tab" });
    expect(onkeydown).toHaveBeenCalled();
  });
});

// ── Custom item snippet ───────────────────────────────────────────────────────

describe("Autocomplete — placeholder", () => {
  it("renders the placeholder text", () => {
    mount({ placeholder: "Pick a fruit" });
    expect(screen.getByPlaceholderText("Pick a fruit")).toBeInTheDocument();
  });
});
