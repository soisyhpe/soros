**TODO :**

First step:
 - [ ] Serveur
   - [ ] Créer un fichier server.rs (src/lib)
     - [ ] Structure Server avec un vecteur de String
	 - [ ] Implanter pour cette structure une méthode new, listen et handle_request
	 - [ ] handle_request doit rajouter un String parsé dans la structure
     - [ ] Sérialisation de la structure à travers le réseau
 - [ ] Client
   - [ ] Afficher la réponse du serveur (String)

Second step:
 - [ ] Regarder la documentation de [**serde**](https://serde.rs) (juste le début)
 - [ ] Créer un nouveau fichier `protocol.rs` avec une enum `Message` qui a : request { okay: u32, okayokay: string }, ok {}, err (String)
   - [ ] Le client envoie une request et le serveur doit lire le contenu de la request
	 - [ ] Match depuis la request (valeur particulière = return ok, sinon err) (PAS LE DROIT D'UTILISER MATCH)
 - [ ] Client : Si ok, print ok, sinon contenu err (ou throw)

