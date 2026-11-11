# OldStar : protocole entre la page et le serveur

La page (`apps/oldstar/web`) est servie par l'exécutable OldStar sur le port `http_port` des
réglages (8080 par défaut). Tout passe par ce seul port : fichiers, WebSocket, long-polling,
images des fenêtres. Le JSON des événements et de l'instantané est celui que fabrique
`src/hub.rs`.

Tout le réseau temps réel de la page est dans `web/js/transport.js` : pour changer de
transport, on ne touche qu'à ce fichier.

## Fichiers

| Requête | Réponse |
|---|---|
| `GET /` | `web/index.html` |
| `GET /oldstar.css`, `GET /js/*.js` | fichiers de `web/` (à embarquer dans l'exe avec `include_str!`) |
| `GET /frame?w=<fenêtre>[&z=<zone>]&n=<compteur>` | image JPEG de la fenêtre `w` (ou de sa zone `z`), `Cache-Control: no-store` ; 404 si la fenêtre est introuvable. `n` ne sert qu'à éviter le cache. |

Les indices `w` et `z` sont ceux des listes `windows` et `zones` de l'instantané.

## Messages du serveur vers la page

Trois formes, identiques en WebSocket et en long-polling :

```json
{ "t": "snap", "seq": 812, "d": { …instantané de Hub::snapshot()… } }
{ "t": "ev",   "seq": 815, "d": [ {"e":"p", …}, {"e":"ys", …} ] }
{ "t": "mtr",  "ms": 4410, "d": { "i": [0..100 par voie], "m": [0..100 par mix] } }
```

- `seq` : numéro du dernier événement contenu (celui de `Hub::events_since`). La page le
  renvoie pour reprendre après une coupure.
- `ms` : numéro du relevé des vu-mètres (`Hub::meters_since`).
- Événements (`d` de `ev`), tels que `hub.rs` les pousse :
  - `{"e":"p","a":adresse,"x":x,"y":y,"v":valeur,"s":texte}` : paramètre de la console ;
  - `{"e":"ys","connected":bool,"model":texte,"error":texte}` : état de la console ;
  - `{"e":"of","t":cible,"a":adresse,"v":valeur}` : retour d'état OSC ;
  - `{"e":"os","alive":[bool…]}` : applis qui répondent ;
  - `{"e":"cfg"}` : réglages rechargés, la page redemande l'instantané.

Si la page a trop de retard (`events_since` rend `None`), le serveur envoie un `snap` au lieu
d'un `ev`.

## Commandes de la page vers le serveur

Objets JSON, en texte WebSocket ou en corps de `POST /cmd` :

| Commande | Effet côté serveur |
|---|---|
| `{"c":"hello","seq":n,"meters":bool,"id":texte}` | premier message WebSocket. `seq` = 0 ou trop ancien : envoyer un `snap`, sinon un `ev` avec ce qui manque. |
| `{"c":"snap"}` | renvoyer un `snap`. |
| `{"c":"meters","on":bool}` | envoyer ou non les relevés `mtr` à ce client (seulement quand l'onglet Console est affiché). |
| `{"c":"set","a":adresse,"x":x,"y":y,"v":valeur}` | `yamaha::Out::Set` (la valeur est déjà brute : centièmes de dB, 0/1, Hz × `freq_scale`, Q × `q_scale`). |
| `{"c":"load","ch":voie}` | `yamaha::Out::LoadChannel` (EQ et envois d'une voie). |
| `{"c":"osc","t":cible,"b":bouton}` | `osc::send` du bouton `b` de la cible `t` des réglages (adresse et arguments pris dans les réglages, jamais dans la page). |
| `{"c":"input","w":fenêtre,"z":zone ou null,"k":"down"/"move"/"up"/"wheel","x":0..1,"y":0..1,"wheel":crans}` | `Frames::input`. |

## WebSocket

`GET /ws` avec `Upgrade: websocket` (RFC 6455, trames texte, ping/pong). Le serveur envoie un
`ev` dès qu'il y a du nouveau (attente sur `Hub::wait`), et un `mtr` au plus `meter_fps` fois
par seconde aux clients qui l'ont demandé. Compter le client dans `Hub::ws_clients`.

## Long-polling (repli)

La page y passe toute seule si `WebSocket` manque, ou s'il échoue trois fois de suite sans
jamais avoir marché.

- `GET /poll?since=<seq>&ms=<ms>&meters=0|1&id=<client>&_=<anti-cache>` : le serveur répond
  tout de suite s'il a du nouveau, sinon attend jusqu'à 20 s (`Hub::wait`). Réponse :
  `{"msgs":[ …messages ci-dessus… ]}` (éventuellement vide). Avec `meters=1`, la réponse part
  dès qu'un relevé plus récent que `ms` existe. Appeler `Hub::poll_seen(id)`.
- `POST /cmd?id=<client>` avec une commande JSON : réponse `204` (ou `200` vide).
