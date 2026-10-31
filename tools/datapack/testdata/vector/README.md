# Vecteur de référence des paquets de données

Publication complète d'un paquet de test (patch `0.4.1`, canal `stable`, séquence 100),
produite par `python tests/test_datapack.py --regen` avec une graine ed25519 **publique
et fixe** (octets 0x00 à 0x1f). Ne jamais utiliser cette clé ailleurs que dans les tests.

- Clé publique (base64) : `A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg=`
- key_id : `56475aa75463474c`

Le vérificateur Rust de l'application doit accepter ce dossier tel quel, et rejeter
toute altération d'un octet (canal, manifeste ou blob). Le test Python vérifie que le
vecteur reste reproductible octet pour octet.
