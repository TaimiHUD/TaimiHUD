## Commun

height = Taille
font = Police
okay = OK
delete = Supprimer
copy = Copier
copy-arg = Copier { $arg }
save = Sauvegarder
quit = Quitter
delete-item = { delete } « { $item } » ?
save-item = { save } « { $item } » ?
save-standalone = { save } dans un nouveau fichier
save-append = Ajouter à un fichier existant
save-edit = { save } les changements
save-edit-item = { save-edit } dans « { $item } » ?
save-mode = Mode de sauvegarde
error = Erreur
remove = Supprimer
unknown = Inconnu
update = Mise à jour
auto-update = Mise à jour automatique
always = Toujours
ask = Demander
never = Jamais
update-allow = Autoriser une fois
unset = Non défini
add = Ajouter
create-arg = Créer une nouvelle { $arg }
not-create-arg = Utiliser une { $arg } existante
description = Description
location = Emplacement : { $path }
data = Donnée
object = Objet
files = Fichiers
clear = Effacer
refresh = Actualiser
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
id-arg = { id } : { $id }
map-id = Carte ({ id })
map-id-arg = { map-id } : { $id }
author = Auteur.rice
position = position
position_cap = Position
not-applicable = N.C
rt-api-required-base = RTAPI est nécessaire pour
rt-api-required = { rt-api-required-base } { $reason }.
no-description = Aucune description.
no-thing-arg = Aucune { $thing } fournie.
expand-all = Tout développer
filetype = Type de fichier
filename = Nom du fichier
collapse-all = Tout réduire
active = Actif.ve
inactive = Inactif.ve
enable = Activer
cancel = Annuler
default = Défaut
disable = Désactiver
enabled = Activé
disabled = Désactivé
author-arg = { author } : { $author }
reset = Réinitialiser
timer = Minuteur
timers = { timer }s
experimental-notice = Salut ! Cette fonctionnalité est (en grande partie) expérimentale.
 Certaines choses peuvent prêter à confusion et son utilisation peut demander plus d'effort que les fonctionnalités moins expérimentales.
 Mes excuses pour tout problème que vous pourriez rencontrer ; n'hésitez pas à rejoindre le Discord pour demander de l'aide.
 - Kat
experimental-notice-alpha = Cette fonctionnalité est (assez) expérimentale. Si vous souhaitez nous aider à la tester, une pré-version plus fonctionnelle (et probablement instable !) est disponible dans le cadre de notre test alpha en cours.
 Visitez taimihud.com/testers pour plus d'informations.
name-empty = Nom vide.
no-trigger = Aucune position d'activation fournie.
no-category = Aucune catégorie fournie.
map-id-wrong = ID de la carte incorrect.
no-positions = Aucune position de marqueur fournie.
validation-fail = Validation échouée à cause de :
filename-empty = Aucun nom de fichier fourni.
count = Compte
#NOTE : count semble inutilisé dans la version actuelle.
actions = Actions
module = Module
unspecified = Non spécifié

## Module d'extension

addon = Extension
primary-window-toggle = Basculer la fenêtre Taimi
context-menu-primary = { menu }
timer-window-toggle = Basculer la fenêtre des minuteurs
marker-window-toggle = Basculer la fenêtre des marqueurs
pathing-window-toggle = Basculer la fenêtre de cheminement
pathing-render-toggle = Basculer l'affichage du cheminement
pathing-render-minimap-toggle = Basculer le cheminement de la mini-carte
pathing-render-map-toggle = Basculer le cheminement de la carte
primary-window-toggle-text = Afficher/Masquer la fenêtre principale de Taimi
timer-key-trigger = Touche de déclenchement du minuteur { $id }
timer-key-reset = Réinitialiser les minuteurs

## Config

config-tab = Configuration
config-advanced = Paramètres avancés
stock-imgui-progress-bar = Barre de progression ImGui par défaut
shadow = Ombrage
centre-text-after-icon = Centrer le texte après l'icône
imgui-notice = Vous pouvez effectuer un Ctrl + Clic gauche sur un curseur (ou autre élément similaire) pour y saisir directement des valeurs.
 N'oubliez pas d'appuyer sur Entrée pour confirmer la valeur.
context-click-notice = Clic droit pour plus d'options
dpi-scaling = Échelle DPI
dpi-notice = Veiller à ce que ceci corresponde à la valeur « {dpi-scaling} » dans les options graphiques du jeu afin que les éléments de la carte s'affichent correctement.
marker-trigger = Déclenchement des marqueurs
marker-condition = Condition du déclenchement
autoplace-warning = Si vous n'avez pas RTAPI d'installé, nous ne pourrons pas vérifier si vous êtes lieutenant ou commandant.
nexus-quick-access = Icônes d'accès rapide
icon-style = Style d'icônes
icon-style-plain = Simple
icon-style-scanlines-1 = Lignes de balayage
preferred-loader = Préférence du chargeur
preferred-updater = Préférence de l'hôte des mises à jour
update-revert-mainline = Revenir aux versions de la branche principale
update-revert-mainline-notice-nexus = Choisissez entre les versions stables et préversions bêta dans les paramètres de Nexus
update-nexus-notice = Les mises à jour en jeu nécessitent Nexus. La gestion des mises à jour automatiques peut être configurée dans les paramètres d'extension d'ArcDPS ou de Nexus.
update-nexus-provider-notice = Vous pouvez gérer les mises à jour dans les options de configuration de l'extension dans Nexus, mais aussi désactiver « { preferred-updater } » pour un contrôle manuel.
gh-api-token = Jeton API GitHub
gh-api-token-notice = Il est possible d'éviter les erreurs liées à la limite de requêtes (Rate Limit) lors de la mise à jour des sources de données en configurant un jeton personnalisé. Ne le faites que si vous comprenez les implications de cette action !
language = Langue
language-unsupported = Aucune traduction
language-help = Vous parlez une langue que nous ne prenons pas encore en charge ?
language-help-calltoaction = Si vous souhaitez aider à traduire TaimiHUD dans d'autres langues pour vos compatriotes et les locuteurs de votre région linguistique, n'hésitez pas à nous contacter !
language-help-info = Localisation de TaimiHUD
language-help-link = "https://taimihud.com/docs/i18n/"
addonbinds = Raccourcis
gamebinds = Raccourcis clavier du jeu
keybind = Raccourci clavier
gamebind-notice = Configurez ces paramètres afin qu'ils correspondent aux raccourcis du jeu. Ils peuvent être détectés automatiquement si arcdps-unofficial-extras est installé.
precise-markers = Marqueurs de position
bind = Associer
press-key = Appuyez sur une touche
config-paths = Contrôles du cheminement
config-paths-toggle-mode = Visibilité du basculement de granularité
config-paths-toggle-mode-notice = Ces modes s'appliquent aux raccourcis clavier que vous avez configurés ainsi qu'aux clics sur les icônes d'accès rapide de Nexus, mais ne changent pas le fonctionnement des menus sur les boutons de bascule avec le clic droit.
config-paths-toggle-mode-space = Le basculement du cheminement peut être configuré pour contrôler les deux cartes à la fois, ou s'adapter à la vue actuellement visible : {space} et la mini-carte alternent ensemble une fois en jeu, mais lorsque vous regardez la carte du monde, c'est elle qui est contrôlée à la place.
config-paths-toggle-mode-map = Faire en sorte que le basculement de la carte du monde contrôle aussi l'affichage de la mini-carte, ou seulement la vue que vous regardez actuellement.
config-paths-toggle-mode-space-group = Basculement principal
config-paths-toggle-mode-space-on = Adaptatif
config-paths-toggle-mode-space-off = Séparé (classique)
config-paths-toggle-mode-map-group = Combiné
config-paths-toggle-mode-map-on = { config-paths-toggle-mode-space-on }
config-paths-toggle-mode-map-off = { config-paths-toggle-mode-space-off }
config-paths-toggle-mode-orientation = Orientation

## Config de l'API

api-tab = API
api-notice = Une clé API vous permet d'activer des fonctionnalités personnalisées, comme masquer les marqueurs de succès terminés.
 Les permissions `progression` et `characters` sont recommandées.
api-token-label = Jeton
api-setup-open = Lier le compte
api-link = "https://account.arena.net/applications"
api-status-setup = Ajout de la clé API...
api-refresh = Actualisation...
api-refresh-achievements = Succès
api-refresh-raids = Raids terminés
api-refresh-account = Compte
api-auto-update = Mises à jour automatiques
api-permissions = Permissions

## Fenêtres

primary-window = TaimiHUD
timers-window = Minuteurs de rencontre
markers-window = Marqueurs d'escouade
pathing-window = Packs de cheminement
timer-window = { timers-window }
marker-window = { markers-window }

## Modules

addon-uninstall-modal-title = Désinstaller { $source } ?
addon-uninstall-modal-button = Désinstaller
addon-uninstall-modal-description = Attention ! Ceci supprimera le dossier ainsi que tout son contenu.
delete-markerset-warning = Attention ! Ceci supprimera l'entrée des marqueurs contenue dans le fichier.
overwrite-markerset = Attention ! Ceci écrasera l'entrée des marqueurs contenue dans le fichier.

## Ouvrable

open-button = Ouvrir { $kind }
open-error = { error } en ouvrant { $kind } : { $path }

## Sources de données

intro-to-data-sources = Veillez à actualiser le répertoire avant de vérifier les mises à jour.
data-sources = Sources de données
data-sources-tab = { data-sources }
data-source-repo-update = Actualiser les sources
data-source-repo-update-tooltip = Récupère les sources de données depuis le dépôt en ligne pour afficher les éléments téléchargeables.
checking-for-updates = Vérification des mises à jour...
downloading-update = Téléchargement de la mise à jour...
check-for-updates = Vérifier les mises à jour
check-for-updates-tooltip = Vérifie les mises à jour pour toutes les sources de données.
 Nous ne lançons pas cette vérification automatiquement, afin de vous laisser décider si des requêtes réseau peuvent être effectuées.
checked-for-updates-last = Dernière vérification : { $time }
reload-data-sources = Recharger les données
reload-data-sources-tooltip = Recharge les éléments à partir des sources de données actuellement installées. Utile si vous les avez modifiés dans « sources.toml » !

remote = Contenu distant
update-status = Statut
version-installed = Version installée : { $version }
version-not-installed = Non installé
update-unknown = Statut de mise à jour inconnu : vérifier les mises à jour ?
update-not-required = Aucune mise à jour nécessaire : déjà à jour !
update-available = Nouvelle version disponible : { $version } !
update-error = { error } en mettant à jour : { $error } !
download = Télécharger
install = Installer
attempt-update = Essayer de mettre à jour ?
settings-unloaded = Les paramètres n'ont pas encore été chargés !
available = Disponible
up-to-date = À jour !

## Onglet infos

info-tab = Infos
join-discord = Rejoindre le Discord
discord-link = "https://discord.gg/FycK2nZKQT"
having-issues = Si vous rencontrez des problèmes avec TaimiHUD, n'hésitez pas à nous contacter sur Discord ou via GitHub !
keybind-triggers = Si vous avez besoin d'un raccourci clavier pour déclencher un minuteur, veuillez associer les touches appropriées dans les paramètres du chargeur.
active-timer-phases = Phases de minuteur actives
phase = Phase
# As in, like, "game engine" or "rendering engine" :o
engine = Moteur
ecs-data = Données ECS
object-data = Données d'objet
object-kind = Type d'objet
model-files = Fichiers de modèle
vertices = Vertices
textures = Textures : { $count }
alloc-size = Allocations : { $size }
d3d-textures = Textures D3D : { $count }
size-frag = { $size } { $suffix }
#size-frag-mb = { size-frag(suffix: "MB", size: "$size") }
size-frag-mb = { $size } Mo
size-frag-kb = { $size } Ko

## Arc

arcdps = ArcDPS
arcdps-tab = { arcdps }
nexus = Nexus
multi-addon-host = Tous

## Onglets des marqueurs

reload-markers = Recharger les marqueurs
marker-tab = { marker-window }
pathing-tab = { pathing-config }
marker = Marqueur
markers = { marker }s
markers-place = Placer les marqueurs
marker-set = Ensemble de marqueurs
marker-set-create = Créer un ensemble de marqueurs
marker-set-edit = Modifier l'ensemble de marqueurs
marker-set-delete = Supprimer l'ensemble de marqueurs
scaling-factor = Facteur d'échelle
current-scaling-factor = { scaling-factor } actuel : ({ $x }, { $y })
current-scaling-factor-multiple = { scaling-factor } actuel, en pieds par unité de coordonnée : ({ $x }, { $y })
scaling-factor-reset = { reset } le facteur d'échelle détecté
no-file-associated = Fichier associé introuvable
markers-arg = { markers } : { $count }
marker-type = Type de marqueur
local-header = Local. (XYZ)
map-header = Carte (XY)
screen-header = Écran (XY)
marker-not-on-screen = Hors champ
select-a-marker = Veuillez sélectionner un marqueur à configurer !
marker-filetype-explanation = Il existe trois types de fichiers de marqueurs : celui fourni avec le module intégré à BlishHUD « Commander's Markers », celui utilisé pour distribuer les marqueurs communautaires, et enfin mon propre format, qui reprend le format par ensemble de marqueurs et en fait un fichier unique par ensemble.
no-markers-for-map = Aucun marqueur trouvé pour la carte actuelle.
cant-place-markers = Impossible à placer
autoplacement-disable = Désactiver le placement automatique
autoplacement-enable = Activer le placement automatique
always-do-action = Toujours faire l'action
do-action-if-commander = Faire l'action si commandant
do-action-if-lieutenant = Faire l'action si lieutenant ou commandant
never-do-action = Ne jamais faire l'action
open-markers-window = Ouvrir la fenêtre des marqueurs
place-markers-automatically = Placer les marqueurs automatiquement
do-nothing = Ne rien faire

## Fenêtre des marqueurs

clear-markers = { clear } les marqueurs
clear-spent-autoplace = Réinitialiser le placement automatique

## Fenêtre d'édition des marqueurs

edit-markers = Créer/Modifier les marqueurs
set-map-id = Définir l'ID de la carte actuelle
current-squad-markers = marqueurs de l'escouade actuelle
take-squad-markers = Récupérer les { current-squad-markers }
cannot-take-squad-markers = Impossible de récupérer les { current-squad-markers } : pas dans une escouade.
rt-api-required-squad-markers = { rt-api-required-base } pour récupérer automatiquement la position des marqueurs de l'escouade.
no-position = Aucune position fournie.
trigger = Déclencheur : { $position }
position-plain = { $position }
position-get = Récupérer la { position } actuelle
set-manually = Définir manuellement
manual-position = { position } manuelle
set-manually-save = { save } la { position }
trigger-explanation = Un déclencheur pour un ensemble de marqueurs est une sphère de 15 m
 de rayon dont le centre est la position du déclencheur.

## Onglets du minuteur

reload-timers = Recharger les minuteurs
timer-tab = { timer-window }
source-arg = Source : { $source }
source-adhoc = Source : Ad-hoc
select-a-timer = Veuillez sélectionner un minuteur à configurer !

## Fenêtre du minuteur

no-phases-active = Aucune phase active, aucun minuteur en cours.
reset-timers = Réinitialiser les minuteurs

## Cheminement

pathing = Cheminement
#Note : Le choix a été porté vers Cheminement pour la traduction de pathing (le mot anglais utilisé pour parler du système dans son ensemble : marker, trail, path, POI), balisage était aussi un candidat potentiel mais moins cohérent avec la traduction du dictionnaire. trails a été traduit par Parcours afin de suivre la règle déjà établi par l'ancienne traduction de TacO.
trail = Parcours
poi = POI
space = KatRender
reload-packs = Recharger
unload-packs = Tout décharger
paths-menu-packs = { controls }
filter-options = Options de filtrage
search = Rechercher
pathing-search = { search }
pathing-search-clear = Effacer la barre de recherche et les résultats.
show-filter = Afficher les options de filtrage
hide-filter = Masquer les options de filtrage
current-map = Carte actuelle
ignore-root = Ignorer l'état de la racine
ignore-leaf = Ignorer l'état des feuilles
ignore-branch = Ignorer l'état des branches
show-hidden = Afficher les éléments cachés
show-all = Tout afficher
#off-map = Elsewhere
ignore-whitespace = Ignorer les espaces
case-insensitive = Ignorer la casse
toggle = Activer/Désactiver
pathing-config = Options de cheminement
pathing-config-enable = Cheminement {space}
pathing-config-minimap = Paramètres de la mini-carte
pathing-config-worldmap = Paramètres de la carte
pathing-config-trail-alpha = Opacité
pathing-config-trail-alpha-minimap = Opacité sur la mini-carte
pathing-config-trail-alpha-worldmap = Opacité sur la carte
pathing-config-poi-alpha = Opacité du panneau d'affichage
pathing-config-poi-alpha-minimap = Opacité des POIs (mini-carte)
pathing-config-poi-alpha-worldmap = Opacité des POIs (carte)
pathing-config-trail-scale = Échelle
pathing-config-trail-scale-minimap = Échelle sur la mini-carte
pathing-config-trail-scale-worldmap = Échelle sur la carte
pathing-config-poi-scale = Taille du panneau d'affichage
pathing-config-poi-scale-minimap = Taille des POIs (mini-carte)
pathing-config-poi-scale-worldmap = Taille des POIs (carte)
pathing-config-player-overlap-threshold = Estomper près du personnage
pathing-config-distance-fade-intensity = Intensité
pathing-config-distance-max = Distance
pathing-config-textured = Parcours texturés
pathing-config-textured-minimap = Parcours texturés
pathing-config-textured-worldmap = Parcours texturés
pathing-config-map-open = Fwoom
pathing-config-camera-source = Source de données de la caméra
pathing-config-advanced = Paramètres avancés
pathing-config-trail-notice = Les paramètres de génération des parcours peuvent nécessiter un changement ou un rechargement de carte pour prendre effet. Ils peuvent ne pas fonctionner comme prévu.
pathing-config-trail-y-offset = Décalage vertical
pathing-config-trail-resolution = Résolution des parcours
pathing-config-trail-width = Largeur de base
pathing-config-goggles = Expérience lunettes à rayons X
pathing-config-goggles-notice = Ceci nécessite de configurer l'échantillonnage du rendu sur Native dans les Options graphiques.
pathing-config-filters = Filtres
pathing-config-api-bypass = Contournement de l'API
pathing-config-api-bypass-notice = Affiche temporairement les succès terminés lors de la rejouabilité du contenu avec vos amis
pathing-config-festivals = {festival}s
pathing-config-festival-active = {$festival} (actif)
pathing-config-reset-notice = Clic droit sur n'importe quel curseur pour rétablir sa valeur par défaut.
pathing-config-edge-feather-scale = Éch. du lissage des bords
pathing-config-corner-boundary-scale = Éch. de délimitation des coins
pathing-notice-space = {space} est requis (pour la fonctionnalité de cheminement).
pathing-notice-mumblelink = Si vous rencontrez des saccades ou des ralentissements, essayez de modifier le paramètre de Synchronisation verticale dans les paramètres graphiques du jeu.
pathing-notice-rtapi-missing = RTAPI est une extension distincte qui doit être installée depuis Nexus.
pathing-notice-rtapi = Si vous rencontrez des saccades ou des ralentissements, essayez de modifier le paramètre de Synchronisation verticale ou de passer à MumbleLink.
mumblelink = MumbleLink
rtapi = Nexus RealTime API

## Space

render-unload = Décharger le rendu
render-reload = Recharger le rendu
render-notice-gameplay = Chargez le jeu pour commencer
render-notice-gameplay-initial = Choisissez un personnage pour commencer
render-notice-error = Erreur ! Consultez le journal (.log) dans le dossier de Nexus ou Taimi pour plus de détails
packs-empty = Aucun fichier n'a été chargé
packs-empty-notice = Une fois installés depuis l'onglet { data-sources-tab } ou manuellement, le bouton « Recharger » devrait les détecter !

## Festivals

festival = Festival
halloween = Halloween
wintersday = Hivernel
superadventurefestival = Super Adventure Box
lunarnewyear = Nouvel An Lunaire
festivalofthefourwinds = Festival des Quatre Vents
dragonbash = Foire du Dragon
Miscellaneous_Interact = Interagir

## Raccourcis (voir `default_keybind`s dans src/exports/runtime/bindings/controls.rs)

UI_ShowHideUI = Masquer l'IU
Map_OpenClose = Carte
Map_ZoomIn = Zoom carte +
Map_ZoomOut = Zoom carte -
Map_FloorUp = Étage carte +
Map_FloorDown = Étage carte -
Map_Recenter = Recentrer
gamebind-marker-arrow = Flèche
gamebind-marker-circle = Cercle
gamebind-marker-heart = Cœur
gamebind-marker-square = Carré
gamebind-marker-star = Étoile
gamebind-marker-spiral = Spirale
gamebind-marker-triangle = Triangle
gamebind-marker-x = X
gamebind-marker-clear = Effacer tous les marqueurs
gamebind-marker-location-suffix = {""}
gamebind-marker-object-suffix = {" "}(cible)

## Commun

Squad_Location_Arrow = {gamebind-marker-arrow}{gamebind-marker-location-suffix}
Squad_Location_Circle = {gamebind-marker-circle}{gamebind-marker-location-suffix}
Squad_Location_Heart = {gamebind-marker-heart}{gamebind-marker-location-suffix}
Squad_Location_Square = {gamebind-marker-square}{gamebind-marker-location-suffix}
Squad_Location_Star = {gamebind-marker-star}{gamebind-marker-location-suffix}
Squad_Location_Spiral = {gamebind-marker-spiral}{gamebind-marker-location-suffix}
Squad_Location_Triangle = {gamebind-marker-triangle}{gamebind-marker-location-suffix}
Squad_Location_X = {gamebind-marker-x}{gamebind-marker-location-suffix}
Squad_Object_Arrow = {gamebind-marker-arrow}{gamebind-marker-object-suffix}
Squad_Object_Circle = {gamebind-marker-circle}{gamebind-marker-object-suffix}
Squad_Object_Heart = {gamebind-marker-heart}{gamebind-marker-object-suffix}
Squad_Object_Square = {gamebind-marker-square}{gamebind-marker-object-suffix}
Squad_Object_Star = {gamebind-marker-star}{gamebind-marker-object-suffix}
Squad_Object_Spiral = {gamebind-marker-spiral}{gamebind-marker-object-suffix}
Squad_Object_Triangle = {gamebind-marker-triangle}{gamebind-marker-object-suffix}
Squad_Object_X = {gamebind-marker-x}{gamebind-marker-object-suffix}
Squad_ClearAllLocationMarkers = {gamebind-marker-clear}{gamebind-marker-location-suffix}
Squad_ClearAllObjectMarkers = {gamebind-marker-clear}{gamebind-marker-object-suffix}
