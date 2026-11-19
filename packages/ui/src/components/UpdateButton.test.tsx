import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { updateInfo } from "../fixtures";
import { UpdateButton, UpdateStatus } from "./UpdateButton";

afterEach(cleanup);

describe("UpdateButton", () => {
  it("recherche une mise à jour quand aucune n'est connue", () => {
    const onCheck = vi.fn();
    const onInstall = vi.fn();
    render(<UpdateButton update={{ phase: "idle" }} onCheck={onCheck} onInstall={onInstall} />);
    fireEvent.click(screen.getByText("Rechercher une mise à jour"));
    expect(onCheck).toHaveBeenCalledOnce();
    expect(onInstall).not.toHaveBeenCalled();
  });

  it("devient « Installer et redémarrer » quand une version est disponible", () => {
    const onCheck = vi.fn();
    const onInstall = vi.fn();
    render(<UpdateButton update={{ phase: "available", info: updateInfo }} onCheck={onCheck} onInstall={onInstall} />);
    const bouton = screen.getByText("Installer et redémarrer");
    expect(bouton.getAttribute("title")).toContain(updateInfo.version);
    fireEvent.click(bouton);
    expect(onInstall).toHaveBeenCalledOnce();
    expect(onCheck).not.toHaveBeenCalled();
  });

  it("est désactivé pendant la vérification et l'installation", () => {
    const { rerender } = render(<UpdateButton update={{ phase: "checking" }} />);
    expect((screen.getByText("Vérification…") as HTMLButtonElement).disabled).toBe(true);
    rerender(<UpdateButton update={{ phase: "installing", info: updateInfo, progress: null }} />);
    expect((screen.getByText("Installation…") as HTMLButtonElement).disabled).toBe(true);
  });

  it("permet de vérifier à nouveau quand Augur est à jour ou après une erreur", () => {
    const onCheck = vi.fn();
    const { rerender } = render(
      <UpdateButton update={{ phase: "upToDate", checkedAt: "2026-11-19T12:00:00Z" }} onCheck={onCheck} />,
    );
    fireEvent.click(screen.getByText("Augur est à jour"));
    rerender(<UpdateButton update={{ phase: "error", message: "hors ligne" }} onCheck={onCheck} idleLabel="Réessayer" />);
    fireEvent.click(screen.getByText("Réessayer"));
    expect(onCheck).toHaveBeenCalledTimes(2);
  });
});

describe("UpdateStatus", () => {
  it("annonce la version disponible, sans répéter la version installée", () => {
    render(<UpdateStatus update={{ phase: "available", info: updateInfo }} />);
    expect(screen.getByText(`Augur ${updateInfo.version} est disponible`)).toBeTruthy();
    expect(screen.queryByText(/tu as la/)).toBeNull();
  });

  it("affiche la progression puis les erreurs", () => {
    const { rerender } = render(
      <UpdateStatus update={{ phase: "installing", info: updateInfo, progress: { downloaded: 5, total: 10 } }} />,
    );
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("50");
    rerender(<UpdateStatus update={{ phase: "error", message: "hors ligne" }} />);
    expect(screen.getByText("hors ligne")).toBeTruthy();
  });

  it("ne montre rien sans mise à jour", () => {
    const { container } = render(<UpdateStatus update={{ phase: "upToDate", checkedAt: "2026-11-19T12:00:00Z" }} />);
    expect(container.firstChild).toBeNull();
  });
});
