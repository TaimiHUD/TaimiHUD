## Common

join-discord = Rejoins le Discord
discord-link = "https://discord.gg/dKpaphTMGS"
having-issues = Si vous rencontrez des problèmes avec TaimiHUD, n'hésitez pas à nous contacter sur Discord ou via GitHub !
height = Taille
font = Police
okay = OK
delete = Supprimer
copy = Copier
copy-arg = Copier { $arg }
save = Sauvegarder
quit = Quitter
delete-item = { delete } "{ $item }"?
save-item = { save } "{ $item }"?
save-standalone = { save } sous un nouveau fichier
save-append = Ajouter à un fichier existant
save-edit = { save } les changements
save-edit-item = { save-edit } to "{ $item }"?
save-mode = Mode de sauvegarde
error = Erreur
remove = Supprimer
unknown = Inconnu
update = Mettre à jour
ask = Demander
always = Toujours
never = Jamais
auto-update = Mise à jour automatique
unset = Désactiver #TODO: context?
add = Ajouter
create-arg = Créer nouveau { $arg }
not-create-arg = Utiliser { $arg } existant
description = Description
location = Emplacement: { $path }
data = Donnée #TODO: context?
object = Objet
files = Fichiers
clear = Effacer
refresh-files = Actualiser les fichiers
# as in 3D
model = Modèle
revert = Annuler
close = Fermer
name = Nom
icon = Icône
path = Chemin
title = Titre
menu = Menu
controls = Contrôles
id = ID
category = Catégorie
id-arg = { id }: { $id }
map-id = Carte { id }
map-id-arg = { map-id }: { $id }
author = Auteur.ice
position = position
position_cap = Position
not-applicable = N/A
rt-api-required-base = RTAPI est requis pour
rt-api-required = { rt-api-required-base } { $reason }.
no-description = Pas de description.
no-thing-arg = Pas de { $thing } fourni.e.
expand-all = Tout développer
filetype = Type du Fichier
filename = Nom du Fichier
collapse-all = Tout réduire
active = Actif #TODO: should this be woke?
inactive = Inactif #TODO: should this be woke?
enable = Activer
cancel = Annuler
default = Défaut
disable = Désactiver
enabled = Activé
disabled = Désactivé
author-arg = { author }: { $author }
reset = Réinitialiser
timer = Minuteur
timers = { timer }s
experimental-notice = Salut! Cette fonctionnalité est (en grande partie) experimentale. Certaines choses peuvent être peu ou pas claires, et son utilisation peut nécessiter plus d'efforts que les fonctionnalités moins expérimentales. Je m'excuse pour tout problème que vous pourriez rencontrer ; n'hésitez pas à rejoindre le Discord pour demander de l'aide. - Kat
name-empty = Nom vide.
no-trigger = Pas de position d'activation fournie.
no-category = Pas de catégorie fournie.
map-id-wrong = ID de Carte incorrect.
no-positions = Pas de position de marqueur fournie.
validation-fail = Validation failed due to:
filename-empty = Pas de nom de fichier fourni.
count = Compte #TODO: context?
actions = Actions
module = Module
unspecified = Non spécifié

## Addon

addon = Addon #TODO: keep it english or use module?
primary-window-toggle = Activer/Désactiver Fenêtre Taimi
context-menu-primary = { menu }
timer-window-toggle = Activer/Désactiver Fenêtre Minuteur
marker-window-toggle = Activer/Désactiver Fenêtre Marqueur
pathing-window-toggle = Activer/Désactiver Fenêtre Pathing
pathing-render-toggle = Activer/Désactiver rendu de chemins
pathing-render-minimap-toggle = Activer/Désactiver rendu de chemins mini-carte
pathing-render-map-toggle = Activer/Désactiver rendu de chemins carte
primary-window-toggle-text = Afficher/cacher fenêtre principale taimi
timer-key-trigger = Touche de déclenchement de minuteur { $id }
timer-key-reset = Réinitialiser { timers }

## Config

config-tab = Configuration
stock-imgui-progress-bar = Barre de progression par défaut d'ImGui
shadow = Ombrage #TODO: or just Ombre?
centre-text-after-icon = Centrer texte après icône
imgui-notice = Vous pouvez Ctrl+Clic gauche sur les curseurs, ou autre, pour directement entrer une valeur manuellement. N'oubliez pas d'appuyer sur Entrée pour confirmer la valeur.
context-click-notice = Clic droit pour plus d'options
dpi-scaling = Échelle DPI
dpi-notice = Veiller à ce que ceci corresponde à la valeur "{dpi-scaling}" dans les options graphiques du jeu afin que les éléments de la carte s'affichent correctement.
marker-trigger = Marker set position trigger behaviour
marker-condition = Behaviour condition
autoplace-warning = Si vous n'avez pas RTAPI d'installé, nous ne pourront pas vérifier si vous êtes commandant ou juste lieutenant.
nexus-quick-access = Icônes d'accès rapide
icon-style = Style d'icônes
icon-style-plain = Simple
icon-style-scanlines-1 = Lignes de balayage
preferred-loader = Loader Preference
preferred-updater = Update Host Preference
gh-api-token = Token d'API GitHub
gh-api-token-notice = Rate limit errors when updating datasources may be avoided by configuring a personalized token - only provide if you understand the implications of doing so!
language = Langue
addonbinds = Raccourcis
gamebinds = Raccourcis clavier du jeu
keybind = Raccourci clavier
gamebind-notice = Configurez les raccourcis ici pour qu'ils correspondent aux raccourcis du jeu. La détection des raccourcis du jeu peut être automatique si arcdps-unofficial-extras est installé.
precise-markers = Marqueurs précis

## Windows

primary-window = TaimiHUD
timers-window = Minuteurs de combat de boss #TODO: is this correct
markers-window = Marqueurs d'escouade
pathing-window = Ensembles de chemins #TODO: ensemble or just pack?
# deprecated(?) aliases
timer-window = { timers-window }
marker-window = { markers-window }

## Modals

addon-uninstall-modal-title = Désinstaller { $source } ?
addon-uninstall-modal-button = Désinstaller
addon-uninstall-modal-description = Attention! Ceci supprimera le dossier et tout son contenu.
delete-markerset-warning = Please be careful! This will delete the marker set entry within the file.
overwrite-markerset = Please be careful! This will overwrite the marker set entry within the file.
## Openable

open-button = Ouvrir { $kind }
open-error = { error } en ouvrant { $kind }: { $path }

## Data sources

intro-to-data-sources = Veillez à rafraîchir le répertoire avant de vérifier les mises à jour.
data-sources = Sources de données
data-sources-tab = { data-sources }
data-source-repo-update = Rafraîchir le répertoire des sources
data-source-repo-update-tooltip = Récupérer le répertoire des sources de données pour voir les éléments téléchargeables.
checking-for-updates = Vérification des mises à jour...
downloading-update = Téléchargement de la mise à jour...
check-for-updates = Vérifier les mises à jour
check-for-updates-tooltip = Vérifier les mises à jour de toutes les sources de données. Nous ne le faisons pas automatiquement pour respecter votre choix de faire des requêtes externes ou non.
checked-for-updates-last = Dernière vérification des mises à jour: { $time }
reload-data-sources = Rafraichîr les sources de données
reload-data-sources-tooltip = Rechargez les éléments à partir des sources de données actuellement installées. Utile si vous les avez modifiés !

remote = Distant #TODO: terminology git uses but seems weird in context
update-status = Status de mise à jour
version-installed = Version installée: { $version }
version-not-installed = Pas installé
update-unknown = Status de mise à jour inconnu; vérifier ?
update-not-required = Pas de mise à jour ; déjà à jour !
update-available = Nouvelle version disponible : { $version } !
update-error = { error } en mettant à jour: { $error } !
download = Télécharger
install = Installer
attempt-update = Essayer quand même de mettre à jour ?
settings-unloaded = Les paramètres n'ont pas encore été chargés !
available = Disponible
up-to-date = À jour !

## Info tab

info-tab = Info
keybind-triggers = Si vous avez besoin de déclencheurs de minuterie activés par raccourcis clavier, veuillez associer les touches appropriées dans les paramètres du chargeur.
active-timer-phases = Phases de minuteur actives
phase = Phase
# As in, like, "game engine" or "rendering engine" :o
engine = Moteur
ecs-data = ECS { data }
object-data = { data } d'{ object } 
object-kind = Type d'{ object } 
model-files = { model } Fichiers
vertices = Vertices
textures = Textures: { $count }
alloc-size = Allocations: { $size }
d3d-textures = Textures D3D: { $count }
size-frag = { $size } { $suffix }
#size-frag-mb = { size-frag(suffix: "MB", size: "$size") }
size-frag-mb = { $size } Mo
size-frag-kb = { $size } Ko

## Arc

arcdps = ArcDPS
arcdps-tab = { arcdps }
nexus = Nexus

## Markers tab

reload-markers = Recharger { markers }
marker-tab = { marker-window }
pathing-tab = { pathing-config }
marker = Marqueur
markers = { marker }s
markers-place = Placer { markers }
marker-set = Ensemble de { marker }s
marker-set-create = Créer { marker-set }
marker-set-edit = Modifier { marker-set }
marker-set-delete = Supprimer { marker-set }
scaling-factor = facteur d'échelle
current-scaling-factor = { scaling-factor } actuel: ({ $x }, { $y })
current-scaling-factor-multiple = { scaling-factor } actuel en multiples de pieds par unité continentale : ({ $x }, { $y })
scaling-factor-reset = { reset } le { scaling-factor } détecté
no-file-associated = Fichier associé introuvable
markers-arg = { markers }: { $count }
marker-type = Type de { marker } 
local-header = (XYZ) Local
map-header = (XY) Carte
screen-header = (XY) Écran
marker-not-on-screen = Pas sur l'écran
select-a-marker = Veuillez sélectionner un marqueur à configurer!
marker-filetype-explanation = There are three kinds of markers file, there is the kind that
  comes with the BlishHUD Commander's Markers module (integrated), there is the kind that they use to ship Community Markers and then there is my own format, which takes the per marker set format and makes it a single file per marker set.
no-markers-for-map = Pas de marqueurs trouvés pour la carte actuelle.
cant-place-markers = Impossible à placer
autoplacement-disable = Désactiver placement automatique
autoplacement-enable = Activer placement automatique
always-do-action = Toujours faire l'action
do-action-if-commander = Faire l'action si commandant
do-action-if-lieutenant = Faire l'action si lieutenant ou commandant
never-do-action = Jamais faire l'action
open-markers-window = Ouvrir la fenêtre des marqueurs
place-markers-automatically = Placer les marqueurs automatiquement
do-nothing = Ne rien faire

## Markers window
clear-markers = { clear } { markers }
clear-spent-autoplace = Reset spent auto-placement

## Edit markers window

edit-markers = Créer/modifier marqueurs
set-map-id = Définir ID de carte à (sur?) la carte actuelle
current-squad-markers = marqueurs d'escouade actuels
take-squad-markers = Take from { current-squad-markers }
cannot-take-squad-markers = Cannot take from { current-squad-markers }; pas dans une escouade. #TODO
rt-api-required-squad-markers = { rt-api-required-base } taking squad marker locations automatically.
no-position = Pas de position fournie.
trigger = Déclencheur: { $position } #TODO: context?
position-plain = { $position }
position-get = Récupérer { position } actuelle
set-manually = Définir manuellement
manual-position = { position } manuelle
set-manually-save = { save } { position } manuelle
trigger-explanation = A trigger for a marker set is a 15m radius sphere with its centre at the trigger location.

## Timer tab

reload-timers = Rafraîchir { timers }
timer-tab = { timer-window }
source-arg = Source: { $source }
source-adhoc = Source: Ad-hoc
select-a-timer = Veuillez sélectionner un minuteur à configurer !

## Timer window

no-phases-active = Aucune phase active, aucun minuteur en cours.
reset-timers = { reset } { timers }

## Pathing

pathing = Pathing
trail = Trail #TODO: there are so many translations of this... Chemin, Traînée, Tracé, chemin balisé, trajet
poi = POI/PI #TODO: this is never ever named like that in french.
space = KatRender
reload-packs = Rafraîchir
unload-packs = Unload All
filter-options = Filter Options
searchbar-clear = Clear the search bar and results.
show-filter = Afficher les options de filtrage
hide-filter = Cacher les options de filtrage
current-map = Carte actuelle
ignore-root = Ignore root state
ignore-leaf = Ignore leaf state
ignore-branch = Ignore branch state
show-hidden = Afficher cachés
show-all = Tout afficher #TODO: Afficher tout?
#off-map = Elsewhere
ignore-whitespace = Ignorer espaces
case-insensitive = Ignorer la casse
toggle = Activer/Désactiver
pathing-config = Paramètres Pathing
pathing-config-enable = {space} Pathing (Expérimental)
pathing-config-minimap = Paramètres Mini-carte
pathing-config-worldmap = Paramètres Carte
pathing-config-trail-alpha = Opacité
pathing-config-trail-alpha-minimap = Opacité Mini-carte
pathing-config-trail-alpha-worldmap = Opacité Carte
pathing-config-poi-alpha = Opacité Panneau d'Affichage
pathing-config-poi-alpha-minimap = Opacité POI Mini-carte
pathing-config-poi-alpha-worldmap = Opacité POI
pathing-config-trail-scale = Échelle
pathing-config-trail-scale-minimap = Échelle Mini-carte
pathing-config-trail-scale-worldmap = Échelle Carte
pathing-config-poi-scale = Taille Panneau d'Affichage
pathing-config-poi-scale-minimap = Taille POI Mini-carte
pathing-config-poi-scale-worldmap = Taille POI 
pathing-config-player-overlap-threshold = Estomper près du personnage
pathing-config-distance-fade-intensity = Intensité
pathing-config-distance-max = Distance
pathing-config-textured = Chemins texturés
pathing-config-textured-minimap = Chemins texturés
pathing-config-textured-worldmap = Chemins texturés
pathing-config-map-open = Fwoom
pathing-config-camera-source = Source de Données Caméra
pathing-config-advanced = Paramètres avancés
pathing-config-trail-notice = Les paramètres de génération des chemins peuvent nécessiter un changement ou un rechargement de la carte pour prendre effet, et peuvent ne pas fonctionner comme prévu.
pathing-config-trail-y-offset = Vertical Offset
pathing-config-trail-resolution = Trail Resolution
pathing-config-trail-width = Base Width
pathing-config-goggles = X-ray Goggles Experiment
pathing-config-goggles-notice = Ceci nécessite de configurer l'échantillonage du rendu sur Native dans les Options graphiques.
pathing-config-festivals = {festival}s
pathing-config-festival-active = {$festival} (actif)
pathing-config-reset-notice = Clic-droit sur n'importe quel curseur pour rétablir sa valeur par défaut.
pathing-config-edge-feather-scale = échelle de lissage des bords
pathing-config-corner-boudary-scale = échelle de délimitation d'angle
pathing-notice-space = {space} est requis (for pathing functionality). #TODO: HOW
pathing-notice-mumblelink = si vous remarquez des ralentissements dans le jeu, essayez de modifier le paramètre Synchronisation verticale dans les paramètres graphiques du jeu
pathing-notice-rtapi-missing = RTAPI est un addon séparé qui doit être installé via Nexus
pathing-notice-rtapi = si vous remarquez des ralentissements dans le jeu, essayez de changer la Synchronisation Verticale ou utilisez MumbleLink
mumblelink = MumbleLink
rtapi = Nexus RealTime API

## Space

render-unload = Activer Rendu
render-reload = Recharger Rendu
render-notice-gameplay = Load in to the game to get started
render-notice-gameplay-initial = Choisissez un personnage pour commencer
render-notice-error = Erreur! Consultez le fichier journal dans le dossier de Nexus ou Taimi pour plus de détails
packs-empty = Pas de fichiers chargés.
packs-empty-notice = Une fois installés depuis l'onglet { data-sources-tab } ou manuellement, le bouton "Recharger" devrait les détecter !

## Festivals

festival = Festival
halloween = Halloween
wintersday = Hivernel
superadventurefestival = Super Adventure Box
lunarnewyear = Nouvel An Lunaire
festivalofthefourwinds = Festival des Quatre Vents
dragonbash = Foire du Dragon
