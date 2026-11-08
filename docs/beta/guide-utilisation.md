# Utiliser Augur pendant la bêta fermée

Prérequis : Augur installé ([guide d'installation](guide-installation.md)) et PoE 2 en
mode **fenêtré sans bordure**.

## Ce que fait cette version

| Fonction | État dans la bêta |
|---|---|
| Overlay sur Ctrl+C : lecture de l'objet (nom, base, mods, tiers) | ✅ à tester |
| Import du personnage par code Path of Building | ✅ à tester à partir de la v0.1.5 |
| Overlay : gain de DPS pour **Lightning Bolt** et **Arc** | ✅ à tester à partir de la v0.1.5, chiffre marqué « ≈ » (approximatif) ; en v0.1.4 l'overlay affiche « Aucun personnage importé » |
| Overlay : gain d'EHP (défense) | ⏳ pas encore |
| Overlay : prix estimé | ⏳ pas encore |
| Session de mesures (export des objets, codes PoB, zones) | ✅ à tester |
| Atlas Ledger (temps en map, maps par heure) | ✅ à tester |
| Forge : coût d'un craft (panneau) et prochaine étape du plan dans l'overlay (Ctrl+Alt+C) | ✅ à tester à partir de la v0.1.6 |
| Mises à jour automatiques | ✅ à tester |

Ce tableau est mis à jour à chaque version de la bêta.

## Importer ton personnage (v0.1.5 et suivantes)

1. Dans **Path of Building PoE 2** : **Import/Export Build > Generate**, puis **Copy**.
   Sur pobb.in, utilise le bouton qui copie le **code** (une longue suite de lettres
   commençant souvent par `eN`), pas le lien.
2. Fenêtre principale Augur > carte **Personnage** : colle le code, puis **Importer**.
3. Augur affiche ta classe, ton niveau et la **compétence comparée** : Lightning Bolt ou
   Arc en priorité, même si ce n'est pas la compétence principale du build. Pour une
   autre compétence, l'overlay indique qu'elle n'est pas encore prise en charge.
4. **Changer** importe un nouveau code ; **Oublier** efface le personnage.

Ton build reste sur ton PC. Pense à le réimporter quand tu changes d'équipement.

## L'overlay (Augur Copilot)

1. Lance le jeu, puis Augur (ou l'inverse, peu importe).
2. En jeu, survole un objet et appuie sur **Ctrl+C**. L'overlay s'ouvre près du curseur.
   - **Ctrl+Alt+C** (copie avancée du jeu) fonctionne aussi et affiche les tiers des mods.
3. Pour le fermer : **Échap**, le bouton « fermer » en bas, ou éloigne simplement le
   curseur.

Bon à savoir :

- L'overlay **ne prend jamais le focus** : tu peux continuer à jouer, tes touches vont au
  jeu.
- Augur ne réagit au Ctrl+C **que si le jeu est au premier plan**. Un Ctrl+C dans ton
  navigateur ou Discord ne déclenche rien.
- **Échap** et **Alt+U** ne sont captés par Augur que pendant l'affichage de l'overlay. Tu peux les changer dans **Réglages > Raccourcis de l'overlay** ; Ctrl+C et Ctrl+Alt+C sont fixés par le jeu.
- La ligue des prix poe.ninja de la Forge (Forbidden Rites par défaut) se change dans **Réglages > Ligue des prix**.
  Le reste du temps, ils restent au jeu.
- Avec un personnage importé, l'overlay affiche le **gain ou la perte de DPS** si
  l'objet remplaçait celui que tu portes au même emplacement (pour une bague : le
  meilleur des deux emplacements). Le calcul suit le panneau de compétence du jeu
  (cible sans résistance, mana plein). Le signe **≈** et l'encadré « Calcul approximatif »
  listent ce qui n'est pas encore compté (certains mods, passifs ou supports).
- Si l'overlay affiche **« Objet non reconnu »**, c'est exactement ce qu'on cherche :
  signale-le (voir plus bas) avec le texte de l'objet.

Ce qu'il faut regarder pendant tes tests :

- L'overlay s'affiche-t-il à chaque Ctrl+C, et assez vite (moins d'une demi-seconde) ?
- Le nom, la base et les mods affichés correspondent-ils à l'objet en jeu ?
- Le gain de DPS a-t-il l'air plausible ? Un objet que tu sais meilleur est-il bien
  affiché en hausse ? Note l'objet (Ctrl+C) et le chiffre si ça te semble faux.
- Le jeu perd-il le focus, saccade-t-il, ou une touche part-elle au mauvais endroit ?
- L'overlay est-il bien placé (pas hors de l'écran, pas sur un second écran) ?

## La session de mesures

Elle sert à rassembler des relevés en jeu dans un seul fichier, pour vérifier les calculs
d'Augur contre les chiffres du jeu.

1. Fenêtre principale > **Session de mesures > Démarrer une session de mesures**
   (ou clic droit sur l'icône Augur > **Démarrer une session de mesures**).
2. Pendant la session, Augur garde :
   - chaque objet copié en jeu (Ctrl+C et Ctrl+Alt+C), même ceux qu'il ne comprend pas ;
   - chaque **code d'export Path of Building** ou lien `pobb.in` / `pastebin.com` que tu
     copies (n'importe où, pas seulement en jeu) ;
   - chaque zone traversée (lue dans `Client.txt`, sans les lignes de chat).
3. Les panneaux du jeu (DPS de la compétence, résistances…) ne sont pas lisibles par
   Augur : recopie leurs valeurs dans le champ **Note libre** puis **Ajouter la note**.
   Une note par relevé, avec le contexte (« Arc niveau 20, sans Zenith, DPS affiché 12 345 »).
4. **Arrêter et exporter** écrit deux fichiers dans `Documents\Augur\mesures` :
   - `augur-mesures-<date>.json` : le fichier à envoyer à Max ;
   - `augur-mesures-<date>.md` : la même chose, lisible par un humain.
5. **Copier dans le presse-papier** copie le JSON pour le coller directement (Discord,
   ticket GitHub…). Augur ne copie rien sans ce clic.

Rien n'est envoyé automatiquement : c'est toi qui transmets le fichier.

## Atlas Ledger

Mesure ton rythme de farm à partir du journal du jeu (pas encore la valeur du butin).

1. Vérifie dans la carte **État** que **Client.txt** est trouvé. Sinon, carte
   **Journal du jeu (Client.txt)** : colle le dossier du jeu, par exemple
   `D:\SteamLibrary\steamapps\common\Path of Exile 2`, ou passe le jeu au premier plan pour
   qu'Augur le retrouve.
2. **Atlas Ledger > Démarrer une session de farm**, puis joue normalement.
3. **Arrêter et enregistrer** range la session dans l'historique, sur ton PC.
4. **Étiqueter** une session (palier de waystone, tablettes, mécaniques, note) permet de
   comparer tes stratégies.

À vérifier : le nombre de maps et leur durée correspondent-ils à ce que tu as joué ?
Une map où tu es revenu après un portail compte-t-elle bien pour une seule ?

## Signaler un bug

Le plus simple : un **ticket GitHub** sur
<https://github.com/ShigenoTV/Augur/issues/new/choose>, modèle **« Bug de la bêta »**.
Il faut un compte GitHub (gratuit). Le formulaire te guide.

Sans compte GitHub, envoie directement à Max les mêmes informations :

1. **Version d'Augur** (en haut de la fenêtre principale, par ex. `v0.1.4`) et
   **Windows 10 ou 11**.
2. **Ce que tu as fait**, étape par étape.
3. **Ce que tu attendais** et **ce qui s'est passé**.
4. Pour un objet mal lu : **le texte de l'objet**. En jeu, Ctrl+C sur l'objet puis colle
   dans le message (le texte suffit, pas besoin de capture).
5. Si possible une **capture d'écran** (Win+Maj+S).

Les dépôts et tickets GitHub d'Augur sont **publics** : n'y colle jamais ton mot de passe,
ton `POESESSID` ou ton adresse. Le texte d'un objet, un code PoB ou un export de mesures
ne contiennent rien de sensible (un export indique au plus l'emplacement de `Client.txt`
sur ton disque).
