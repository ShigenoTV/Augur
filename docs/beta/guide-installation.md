# Installer Augur (bêta fermée)

Ce guide s'adresse aux testeurs de la bêta fermée. Compte 5 minutes.

## Ce qu'il te faut

- **Windows 10 ou 11**, 64 bits.
- **Path of Exile 2** installé (Steam ou client autonome).
- Une connexion Internet à la première installation : Windows peut avoir besoin de
  télécharger le composant **WebView2** de Microsoft (déjà présent sur Windows 11 et sur
  la plupart des Windows 10 à jour).

## 1. Télécharger l'installeur

1. Ouvre la page des versions : <https://github.com/ShigenoTV/Augur/releases/latest>.
2. Dans **Assets**, télécharge **`Augur_X.Y.Z_x64-setup.exe`** (X.Y.Z = numéro de version).
   - Le fichier `.msi` fait la même chose ; prends le `.exe` sauf si on te demande le `.msi`.
   - Ignore les fichiers `.sig`, `latest.json` et `SHA256SUMS.txt` : ils servent aux mises
     à jour automatiques et aux vérifications.

### Facultatif : vérifier le fichier

`SHA256SUMS.txt` contient l'empreinte de chaque installeur. Dans PowerShell, depuis ton
dossier Téléchargements :

```powershell
Get-FileHash .\Augur_X.Y.Z_x64-setup.exe -Algorithm SHA256
```

La valeur affichée doit être identique à celle de la ligne `Augur_X.Y.Z_x64-setup.exe`
dans `SHA256SUMS.txt`.

## 2. Passer l'écran « Windows a protégé votre ordinateur »

Augur n'a **pas de certificat de signature de code payant** (choix assumé pour un outil
entre amis). Windows SmartScreen affiche donc un écran bleu au premier lancement de
l'installeur. C'est attendu :

1. Clique sur **Informations complémentaires**.
2. Vérifie que l'éditeur indiqué est **« Éditeur inconnu »** et que le fichier s'appelle
   bien `Augur_X.Y.Z_x64-setup.exe`.
3. Clique sur **Exécuter quand même**.

Si ton antivirus bloque ou met le fichier en quarantaine, ne le désactive pas : préviens
Max (voir [Signaler un bug](guide-utilisation.md#signaler-un-bug)) avec le nom de
l'antivirus et le message exact.

Cet écran n'apparaît qu'à l'installation manuelle. Les mises à jour automatiques ne le
déclenchent pas.

## 3. Installer

L'installeur s'installe **pour ton compte Windows uniquement** : pas besoin de droits
administrateur. Laisse les options par défaut et termine. Augur se lance et apparaît :

- dans une **fenêtre principale** (état, Atlas Ledger, session de mesures, mises à jour) ;
- dans la **zone de notification** (icône à côté de l'horloge, parfois cachée derrière la
  flèche `^`).

Fermer la fenêtre principale ne quitte pas Augur : il reste dans la zone de notification
pour surveiller tes Ctrl+C. Pour le quitter vraiment : clic droit sur l'icône >
**Quitter**.

## 4. Régler le jeu

Dans les options de PoE 2, **Graphismes > Mode d'affichage : Fenêtré sans bordure**
(*Windowed Fullscreen*). En plein écran exclusif, l'overlay ne peut pas s'afficher
par-dessus le jeu.

## 5. Mises à jour

- Augur vérifie les mises à jour **20 secondes après son démarrage**, puis **toutes les
  6 heures**. Tu peux aussi forcer une vérification : fenêtre principale >
  **Mises à jour > Rechercher des mises à jour**, ou clic droit sur l'icône >
  **Rechercher des mises à jour**.
- Quand une version est disponible, un bandeau apparaît dans la fenêtre principale.
  **Rien ne s'installe sans ton clic.** Augur se ferme, s'installe et se relance tout seul.
- Chaque mise à jour est signée par le studio : Augur refuse une mise à jour qui n'a pas
  la bonne signature.
- Si une mise à jour échoue, tu peux toujours réinstaller par-dessus avec le dernier
  installeur de la page des versions (tes réglages et ton historique sont conservés).

## Désinstaller

**Paramètres Windows > Applications > Applications installées > Augur > Désinstaller.**

Tes données restent sur le PC après la désinstallation. Pour tout effacer, supprime aussi :

| Dossier | Contenu |
|---|---|
| `%APPDATA%\com.shigenotv.augur` | Réglages et historique d'Atlas Ledger (`ledger.sqlite`) |
| `Documents\Augur\mesures` | Exports des sessions de mesures |

## Ce qu'Augur fait et ne fait pas sur ton PC

- Il lit le **presse-papier** seulement quand le jeu est au premier plan (ou pendant une
  session de mesures que tu as lancée) et le **journal du jeu** `Client.txt`.
- Il **n'envoie aucune touche au jeu**, ne lit pas sa mémoire et ne modifie aucun de ses
  fichiers : il respecte les règles de GGG sur les outils tiers.
- Il ne te demande **jamais** ton identifiant de session (`POESESSID`) ni ton mot de passe.
- Rien n'est envoyé ailleurs : les exports restent sur ton PC, à toi de les transmettre.
  Seule la vérification des mises à jour contacte GitHub.
