import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { updateInfo } from "../fixtures";
import { UpdateBanner } from "./UpdateBanner";

afterEach(cleanup);

describe("UpdateBanner", () => {
  it("propose d'installer, sans rien forcer", () => {
    const onInstall = vi.fn();
    const onLater = vi.fn();
    render(<UpdateBanner update={{ phase: "available", info: updateInfo }} onInstall={onInstall} onLater={onLater} />);
    expect(screen.getByText(/Augur 0.2.0 est disponible/)).toBeTruthy();
    fireEvent.click(screen.getByText("Plus tard"));
    expect(onLater).toHaveBeenCalledOnce();
    expect(onInstall).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Installer et redémarrer"));
    expect(onInstall).toHaveBeenCalledOnce();
  });

  it("affiche la progression", () => {
    render(
      <UpdateBanner
        update={{ phase: "installing", info: updateInfo, progress: { downloaded: 5_242_880, total: 10_485_760 } }}
      />,
    );
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("50");
  });

  it("ne montre rien quand l'application est à jour", () => {
    const { container } = render(<UpdateBanner update={{ phase: "upToDate", checkedAt: "2026-10-31T15:00:00Z" }} />);
    expect(container.firstChild).toBeNull();
  });
});
