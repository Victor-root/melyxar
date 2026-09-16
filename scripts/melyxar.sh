#!/usr/bin/env bash
set -Eeuo pipefail
umask 022

# ──────────────────────────────────────────────────────────────────────────────
# Melyxar media server: install, update and management.
# The script speaks the language of the system, English or French.
# ──────────────────────────────────────────────────────────────────────────────

REPO_URL="https://github.com/Victor-root/melyxar.git"
BRANCH="develop"
# This script on its own, without the repository around it. Running it from
# here is what keeps it current: a copy saved on the machine is a copy that
# stops being the published one the day it is changed, and nothing says so.
SCRIPT_URL="https://raw.githubusercontent.com/Victor-root/melyxar/${BRANCH}/scripts/melyxar.sh"

APP_USER="melyxar"
APP_GROUP="melyxar"
SOURCE_DIR="/opt/melyxar/source"
BINARY_PATH="/usr/local/bin/melyxar"
CONFIG_DIR="/etc/melyxar"
CONFIG_FILE="${CONFIG_DIR}/melyxar.toml"
DATA_DIR="/var/lib/melyxar"
CACHE_DIR="/var/cache/melyxar"
BACKUP_DIR="${DATA_DIR}/backups"
UNIT_FILE="/etc/systemd/system/melyxar.service"
SERVICE="melyxar"

DEFAULT_PORT="2100"
# Backups kept before the oldest is dropped.
BACKUP_KEEP="7"

# Whether every command shows its own output as it goes.
#
# On while Melyxar is being built, because "it did not work" with nothing to
# show is not a bug report, and a step that succeeds can still have said
# something worth reading. To be turned off once the thing is finished, when a
# tidy screen is what somebody installing it wants. --quiet does it for one
# run, MELYXAR_VERBOSE=0 in the environment for good.
VERBOSE="${MELYXAR_VERBOSE:-1}"

# ── Language ──────────────────────────────────────────────────────────────────
# The script speaks the language of the system, English by default. Same
# mechanism as the other scripts of the author: the locale decides, and English
# is the fallback for an unknown locale as well as for a missing key.

detect_lang() {
  local raw="${LC_ALL:-${LC_MESSAGES:-${LANG:-en}}}"
  raw="${raw,,}"
  case "$raw" in
    fr* ) APP_LANG="fr" ;;
    * ) APP_LANG="en" ;;
  esac
}

APP_LANG="en"
detect_lang

declare -A I18N

load_i18n() {
  local lang key text
  while IFS='|' read -r lang key text; do
    [[ -z "${lang:-}" || -z "${key:-}" ]] && continue
    I18N["$lang:$key"]="$text"
  done <<'I18N_DATA'
en|app_name|Melyxar media server install, update and management
fr|app_name|Installation, mise à jour et gestion du serveur multimédia Melyxar
en|err_need_root|Run this script as root.
fr|err_need_root|Exécutez ce script en root.
en|err_command_output|Command output:
fr|err_command_output|Sortie de la commande :
en|verbose_on|Verbose: every command shows its own output as it runs.
fr|verbose_on|Mode détaillé : chaque commande affiche sa sortie au fur et à mesure.
en|err_aborted|Stopped at line %s. Nothing else was changed.
fr|err_aborted|Arrêt à la ligne %s. Rien d'autre n'a été modifié.
en|err_missing_command|Missing command: %s
fr|err_missing_command|Commande manquante : %s
en|hint_yes_default|Enter=yes / no
fr|hint_yes_default|Entrée=oui / non
en|hint_no_default|yes / Enter=no
fr|hint_no_default|oui / Entrée=non
en|menu_title|What do you want to do?
fr|menu_title|Que voulez-vous faire ?
en|menu_install|Install Melyxar
fr|menu_install|Installer Melyxar
en|menu_update|Update to the latest version
fr|menu_update|Mettre à jour vers la dernière version
en|menu_status|Show the server report
fr|menu_status|Afficher le rapport du serveur
en|menu_backup|Back up the database now
fr|menu_backup|Sauvegarder la base maintenant
en|menu_restore|Restore from a backup
fr|menu_restore|Restaurer depuis une sauvegarde
en|menu_uninstall|Uninstall
fr|menu_uninstall|Désinstaller
en|menu_lines|Count the lines of code
fr|menu_lines|Compter les lignes de code
en|menu_quit|Quit
fr|menu_quit|Quitter
en|prompt_choice|Your choice
fr|prompt_choice|Votre choix
en|err_bad_choice|Unknown choice: %s
fr|err_bad_choice|Choix inconnu : %s
en|section_checks|Checking the system
fr|section_checks|Vérification du système
en|check_debian_ok|Debian based system detected
fr|check_debian_ok|Système basé sur Debian détecté
en|check_debian_warn|This script targets a Debian based system. Continuing anyway.
fr|check_debian_warn|Ce script vise un système basé sur Debian. On continue quand même.
en|check_container|Running inside a container
fr|check_container|Exécution dans un conteneur
en|check_memory|Memory available: %s MB
fr|check_memory|Mémoire disponible : %s Mo
en|warn_memory_low|Building needs roughly 6 to 8 GB at its peak. With less, the build may be killed.
fr|warn_memory_low|La compilation demande environ 6 à 8 Go au pic. En dessous, elle peut être tuée.
en|check_disk|Free space on the system disk: %s GB
fr|check_disk|Espace libre sur le disque système : %s Go
en|warn_disk_low|Building needs roughly 20 GB for its intermediate files.
fr|warn_disk_low|La compilation demande environ 20 Go pour ses fichiers intermédiaires.
en|check_port_free|Port %s is free
fr|check_port_free|Le port %s est libre
en|check_port_busy|Port %s is already in use by something else
fr|check_port_busy|Le port %s est déjà utilisé par autre chose
en|check_ffmpeg_ok|Media tools found: %s
fr|check_ffmpeg_ok|Outils média trouvés : %s
en|check_ffmpeg_missing|Media tools not found. They will be installed.
fr|check_ffmpeg_missing|Outils média absents. Ils seront installés.
en|check_dri_ok|Graphics device visible, hardware acceleration is possible
fr|check_dri_ok|Périphérique graphique visible, l'accélération matérielle est possible
en|check_dri_missing|No graphics device visible
fr|check_dri_missing|Aucun périphérique graphique visible
en|dri_title|Hardware acceleration needs a device
fr|dri_title|L'accélération matérielle a besoin d'un périphérique
en|dri_line1|Melyxar can use a graphics card to transcode, which costs far less
fr|dri_line1|Melyxar peut utiliser une carte graphique pour transcoder, ce qui coûte bien
en|dri_line2|processor time than doing it in software.
fr|dri_line2|moins de temps processeur que de le faire en logiciel.
en|dri_line3|In an unprivileged container the device has to be passed from the host.
fr|dri_line3|Dans un conteneur non privilégié, le périphérique doit être passé depuis l'hôte.
en|dri_line4|This is done on the host, not inside the container.
fr|dri_line4|Cette action se fait sur l'hôte, pas dans le conteneur.
en|dri_commands|Commands to run on the Proxmox host:
fr|dri_commands|Commandes à lancer sur le host Proxmox :
en|dri_replace|Replace <CTID> with the container id.
fr|dri_replace|Remplacez <CTID> par l'identifiant du conteneur.
en|dri_later|You can install now and pass the device later.
fr|dri_later|Vous pouvez installer maintenant et passer le périphérique plus tard.
en|section_packages|Installing what is needed
fr|section_packages|Installation des dépendances
en|step_apt_update|Refreshing the package lists
fr|step_apt_update|Rafraîchissement de la liste des paquets
en|step_apt_install|Installing build tools and media tools
fr|step_apt_install|Installation des outils de compilation et des outils média
en|step_rust|Installing the Rust toolchain
fr|step_rust|Installation de la chaîne d'outils Rust
en|rust_present|Rust toolchain already present: %s
fr|rust_present|Chaîne d'outils Rust déjà présente : %s
en|section_account|Preparing the system account and folders
fr|section_account|Préparation du compte système et des dossiers
en|step_user|Creating the system account
fr|step_user|Création du compte système
en|user_present|System account already present
fr|user_present|Compte système déjà présent
en|step_dirs|Creating the folders
fr|step_dirs|Création des dossiers
en|section_source|Getting the source
fr|section_source|Récupération des sources
en|step_clone|Cloning the repository, with its full history
fr|step_clone|Clonage du dépôt, avec tout son historique
en|step_pull|Fetching the latest changes
fr|step_pull|Récupération des dernières modifications
en|step_unshallow|Completing a partial copy of the repository into a full one
fr|step_unshallow|Complétion d'une copie partielle du dépôt en copie complète
en|section_update_diff|What is about to change
fr|section_update_diff|Ce qui va changer
en|update_diff_up_to_date|Already at the latest version. Nothing to update.
fr|update_diff_up_to_date|Déjà à la dernière version. Rien à mettre à jour.
en|section_lines|Lines of code
fr|section_lines|Lignes de code
en|step_fetch_main|Fetching the %s branch as well
fr|step_fetch_main|Récupération de la branche %s également
en|lines_on|%s: %s lines
fr|lines_on|%s : %s lignes
en|lines_ahead|%s is %s lines longer than %s
fr|lines_ahead|%s compte %s lignes de plus que %s
en|lines_behind|%s is %s lines shorter than %s
fr|lines_behind|%s compte %s lignes de moins que %s
en|lines_same|%s and %s are the same length
fr|lines_same|%s et %s font la même longueur
en|lines_changed|%s lines added, %s removed across %s files
fr|lines_changed|%s lignes ajoutées, %s supprimées sur %s fichiers
en|lines_counted|Counted without the built interface, the lock files and the images.
fr|lines_counted|Compté sans l'interface compilée, les fichiers de verrouillage et les images.
en|section_build|Building
fr|section_build|Compilation
en|build_notice|This takes 5 to 10 minutes the first time and 1 to 2 minutes afterwards.
fr|build_notice|Cela prend 5 à 10 minutes la première fois, puis 1 à 2 minutes ensuite.
en|build_nice|The build runs at low priority, so a film playing now will not stutter.
fr|build_nice|La compilation tourne en priorité basse, une lecture en cours ne saccadera pas.
en|step_build|Building the server
fr|step_build|Compilation du serveur
en|step_install_binary|Installing the binary
fr|step_install_binary|Installation du binaire
en|section_config|Configuration
fr|section_config|Configuration
en|config_present|A configuration file already exists and was left untouched
fr|config_present|Un fichier de configuration existe déjà et n'a pas été touché
en|step_config|Writing the starting configuration
fr|step_config|Écriture de la configuration de départ
en|prompt_port|Which port should Melyxar listen on
fr|prompt_port|Sur quel port Melyxar doit-il écouter
en|prompt_add_library|Add a library now
fr|prompt_add_library|Ajouter une bibliothèque maintenant
en|prompt_library_name|Library name
fr|prompt_library_name|Nom de la bibliothèque
en|prompt_library_kind|Library kind (movies, series, anime, shows, music)
fr|prompt_library_kind|Type de bibliothèque (movies, series, anime, shows, music)
en|prompt_root_path|Folder holding the media (empty to stop adding)
fr|prompt_root_path|Dossier contenant les médias (vide pour arrêter d'ajouter)
en|prompt_root_label|Short label for this folder, shown in logs instead of the path
fr|prompt_root_label|Libellé court pour ce dossier, affiché dans les journaux au lieu du chemin
en|root_added|Folder added: %s
fr|root_added|Dossier ajouté : %s
en|root_missing|That folder does not exist. Add it anyway?
fr|root_missing|Ce dossier n'existe pas. L'ajouter quand même ?
en|root_unreadable|That folder exists but cannot be read by the server account.
fr|root_unreadable|Ce dossier existe mais ne peut pas être lu par le compte du serveur.
en|root_not_absolute|A folder has to be given in full, starting with a slash.
fr|root_not_absolute|Un dossier doit être donné en entier, en commençant par une barre oblique.
en|err_bad_kind|Unknown kind. Pick one of: movies, series, anime, shows, music.
fr|err_bad_kind|Type inconnu. Choisissez parmi : movies, series, anime, shows, music.
en|library_without_root|No folder was given, so the library was not written. You can add it later from the interface.
fr|library_without_root|Aucun dossier n'a été donné, la bibliothèque n'a donc pas été écrite. Vous pourrez l'ajouter plus tard depuis l'interface.
en|section_service|Service
fr|section_service|Service
en|step_unit|Installing the service file
fr|step_unit|Installation du fichier de service
en|step_enable|Enabling the service at boot
fr|step_enable|Activation du service au démarrage
en|step_restart|Restarting the service
fr|step_restart|Redémarrage du service
en|step_stop|Stopping the service
fr|step_stop|Arrêt du service
en|section_done|Done
fr|section_done|Terminé
en|done_title|Melyxar is running
fr|done_title|Melyxar tourne
en|done_open|Open it at:
fr|done_open|Ouvrez-le à l'adresse :
en|done_config|Configuration file:
fr|done_config|Fichier de configuration :
en|done_logs|Follow the logs with:
fr|done_logs|Suivez les journaux avec :
en|done_report|Run the report with:
fr|done_report|Lancez le rapport avec :
en|done_again|Run this script again, always the published version, with:
fr|done_again|Relancez ce script, toujours dans sa version publiée, avec :
en|section_backup|Backup
fr|section_backup|Sauvegarde
en|step_backup|Copying the database
fr|step_backup|Copie de la base
en|backup_done|Backup written to %s
fr|backup_done|Sauvegarde écrite dans %s
en|backup_none|No database to back up yet
fr|backup_none|Aucune base à sauvegarder pour l'instant
en|section_restore|Restore
fr|section_restore|Restauration
en|restore_none|No backup found in %s
fr|restore_none|Aucune sauvegarde trouvée dans %s
en|restore_pick|Which backup should be restored
fr|restore_pick|Quelle sauvegarde faut-il restaurer
en|restore_confirm|This replaces the current database. A copy of it is kept first. Continue?
fr|restore_confirm|Cela remplace la base actuelle. Une copie est gardée avant. Continuer ?
en|step_restore|Putting the backup back in place
fr|step_restore|Remise en place de la sauvegarde
en|restore_done|Restored. The current database was kept as %s
fr|restore_done|Restauré. La base actuelle a été gardée sous %s
en|restore_done_fresh|Restored. There was no database to keep aside.
fr|restore_done_fresh|Restauré. Il n'y avait aucune base à mettre de côté.
en|section_uninstall|Uninstall
fr|section_uninstall|Désinstallation
en|uninstall_confirm|Remove the service, the binary and the sources?
fr|uninstall_confirm|Supprimer le service, le binaire et les sources ?
en|uninstall_keep_data|Your library data and configuration are kept. Remove them too?
fr|uninstall_keep_data|Vos données de bibliothèque et la configuration sont gardées. Les supprimer aussi ?
en|uninstall_data_kept|Data kept in %s and configuration in %s
fr|uninstall_data_kept|Données gardées dans %s et configuration dans %s
en|uninstall_done|Melyxar was removed
fr|uninstall_done|Melyxar a été supprimé
en|err_not_installed|Melyxar is not installed yet. Choose install first.
fr|err_not_installed|Melyxar n'est pas encore installé. Choisissez d'abord l'installation.
en|err_no_terminal|There is no terminal to ask on. Name the action: install, update, status, backup, restore, uninstall.
fr|err_no_terminal|Aucun terminal pour poser les questions. Indiquez l'action : install, update, status, backup, restore, uninstall.
en|cancelled|Cancelled
fr|cancelled|Annulé
I18N_DATA
}

load_i18n

tr_msg() {
  local key="$1"
  printf '%s\n' "${I18N["$APP_LANG:$key"]:-${I18N["en:$key"]:-$key}}"
}

# Same, for a message carrying values: the text holds %s placeholders.
tr_fmt() {
  local key="$1"
  shift
  # shellcheck disable=SC2059
  printf "${I18N["$APP_LANG:$key"]:-${I18N["en:$key"]:-$key}}" "$@"
}

APP_NAME="$(tr_msg app_name)"

# ── Colours ───────────────────────────────────────────────────────────────────

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
  RED=$'\033[38;5;160m'
  RED_DARK=$'\033[38;5;124m'
  RED_SOFT=$'\033[38;5;203m'
  AMBER=$'\033[38;5;214m'
  GREEN=$'\033[38;5;120m'
  CYAN=$'\033[38;5;117m'
  GRAY=$'\033[38;5;245m'
  DIM=$'\033[2m'
  BOLD=$'\033[1m'
  RESET=$'\033[0m'
else
  RED=""
  RED_DARK=""
  RED_SOFT=""
  AMBER=""
  GREEN=""
  CYAN=""
  GRAY=""
  DIM=""
  BOLD=""
  RESET=""
fi

SPINNER_FRAMES=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")

# ── UI ────────────────────────────────────────────────────────────────────────

term_width() {
  local cols
  cols="$(tput cols 2>/dev/null || echo 80)"
  [[ -z "$cols" || "$cols" -lt 50 ]] && cols=80
  [[ "$cols" -gt 92 ]] && cols=92
  echo "$cols"
}

hr() {
  local cols line
  cols="$(term_width)"
  # Bash substitution instead of tr: tr works on bytes and would turn a
  # multi byte box character into garbage.
  printf -v line "%*s" "$cols" ""
  printf "%b%s%b\n" "${RED_DARK}" "${line// /─}" "${RESET}"
}

success() { printf "%b✓%b %b\n" "${GREEN}" "${RESET}" "$*"; }
info()    { printf "%b›%b %b\n" "${RED_SOFT}" "${RESET}" "$*"; }
warn()    { printf "%b⚠%b %b\n" "${AMBER}" "${RESET}" "$*"; }
error()   { printf "%b✗%b %b\n" "${RED}" "${RESET}" "$*" >&2; }
die()     { error "$*"; exit 1; }

# A step that fails stops the script, and saying so beats a silent exit.
on_error() {
  local rc=$?
  trap - ERR
  error "$(tr_fmt err_aborted "${BASH_LINENO[0]}")"
  exit "$rc"
}
trap on_error ERR

section() {
  echo
  printf "%b%s%b\n" "${BOLD}${RED}" "$1" "${RESET}"
  hr
}

panel() {
  local color="$1"
  local title="$2"
  shift 2

  echo
  printf "%b┌%b %b%b%s%b\n" "$color" "$RESET" "$BOLD" "$color" "$title" "$RESET"
  while (($#)); do
    printf "%b│%b %b\n" "$color" "$RESET" "$1"
    shift
  done
  printf "%b└%b\n" "$color" "$RESET"
}

banner() {
  clear 2>/dev/null || true
  echo
  # Le Y est rapproché du L de trois colonnes. Le L est creux en haut à
  # droite, le Y l'est en bas à gauche, et laissés à leur chasse naturelle les
  # deux creux se font face : sept colonnes de vide au plus large, là où les
  # autres lettres se touchent. Trois est le maximum, au-delà le pied du Y
  # entre dans celui du L.
  printf "%b%s%b\n" "${RED}" "███╗   ███╗███████╗██╗  ██╗   ██╗██╗  ██╗ █████╗ ██████╗ " "${RESET}"
  printf "%b%s%b\n" "${RED}" "████╗ ████║██╔════╝██║  ╚██╗ ██╔╝╚██╗██╔╝██╔══██╗██╔══██╗" "${RESET}"
  printf "%b%s%b\n" "${RED}" "██╔████╔██║█████╗  ██║   ╚████╔╝  ╚███╔╝ ███████║██████╔╝" "${RESET}"
  printf "%b%s%b\n" "${RED}" "██║╚██╔╝██║██╔══╝  ██║    ╚██╔╝   ██╔██╗ ██╔══██║██╔══██╗" "${RESET}"
  printf "%b%s%b\n" "${RED}" "██║ ╚═╝ ██║███████╗███████╗██║   ██╔╝ ██╗██║  ██║██║  ██║" "${RESET}"
  printf "%b%s%b\n" "${RED}" "╚═╝     ╚═╝╚══════╝╚══════╝╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝" "${RESET}"
  echo
  printf "  %b%s%b %b· by Victor-root%b\n" "${BOLD}${RED_SOFT}" "$APP_NAME" "${RESET}" "${GRAY}" "${RESET}"
  hr
}

step() {
  local label="$1"
  shift

  local log
  log="$(mktemp)"
  local rc=0

  if [[ "$VERBOSE" -eq 1 ]]; then
    # Everything on screen as it happens, the command included. No spinner:
    # it would fight with the output for the same line.
    printf "%b▸%b %s\n" "${RED_SOFT}" "${RESET}" "$label"
    printf "%b  $ %s%b\n" "${GRAY}" "$*" "${RESET}"
    # Indented so what a command says is never mistaken for what the script
    # says. The status read is the command's own, not the one the pipe ends on.
    set +e
    "$@" 2>&1 | tee "$log" | sed -e 's/^/    /'
    rc="${PIPESTATUS[0]}"
    set -e
    if [[ "$rc" -eq 0 ]]; then
      printf "%b✓%b %s\n" "${GREEN}" "${RESET}" "$label"
    else
      printf "%b✗%b %s\n" "${RED}" "${RESET}" "$label"
    fi
    # Already on screen, so it is not printed a second time below.
    rm -f "$log"
    return "$rc"
  fi

  if [[ -t 1 ]]; then
    "$@" >"$log" 2>&1 &
    local pid=$!
    local i=0
    while kill -0 "$pid" 2>/dev/null; do
      printf "\r%b%s%b %b%s…%b" \
        "${RED_SOFT}" "${SPINNER_FRAMES[i]}" "${RESET}" \
        "${DIM}" "$label" "${RESET}"
      i=$(((i + 1) % ${#SPINNER_FRAMES[@]}))
      sleep 0.08
    done
    wait "$pid" || rc=$?
    if [[ "$rc" -eq 0 ]]; then
      printf "\r\033[2K%b✓%b %s\n" "${GREEN}" "${RESET}" "$label"
    else
      printf "\r\033[2K%b✗%b %s\n" "${RED}" "${RESET}" "$label"
    fi
  else
    printf "%b▸%b %s\n" "${RED_SOFT}" "${RESET}" "$label"
    "$@" >"$log" 2>&1 || rc=$?
    if [[ "$rc" -eq 0 ]]; then
      printf "%b✓%b %s\n" "${GREEN}" "${RESET}" "$label"
    else
      printf "%b✗%b %s\n" "${RED}" "${RESET}" "$label"
    fi
  fi

  if [[ "$rc" -ne 0 ]]; then
    echo
    error "$(tr_msg err_command_output)"
    sed -e 's/^/  /' "$log" >&2
    rm -f "$log"
    return "$rc"
  fi

  rm -f "$log"
  return 0
}

prompt_label() {
  local color="$1"
  local label="$2"
  local hint="${3:-}"

  if [[ -n "$hint" ]]; then
    printf "%b?%b %b%s%b %b[%s]%b : " \
      "$color" "${RESET}" \
      "${BOLD}" "$label" "${RESET}" \
      "${DIM}" "$hint" "${RESET}" >&2
  else
    printf "%b?%b %b%s%b : " \
      "$color" "${RESET}" \
      "${BOLD}" "$label" "${RESET}" >&2
  fi
}

has_terminal() {
  [[ -t 0 ]] && return 0
  if { exec 3</dev/tty; } 2>/dev/null; then
    exec 3<&-
    return 0
  fi
  return 1
}

# The answer is read from the terminal when the script itself arrives on the
# standard input, and from nowhere at all in a scheduled job, where an end of
# file gives an empty answer and the default applies instead of the run
# failing.
read_answer() {
  local value=""

  if [[ -t 0 ]]; then
    IFS= read -r value || value=""
  elif { exec 3</dev/tty; } 2>/dev/null; then
    IFS= read -r value <&3 || value=""
    exec 3<&-
  fi

  printf "%s" "$value"
}

is_yes() {
  local value="${1,,}"
  [[ "$value" == "oui" || "$value" == "o" || "$value" == "yes" || "$value" == "y" ]]
}

prompt_default() {
  local value
  prompt_label "${RED_SOFT}" "$1" "$2"
  value="$(read_answer)"
  printf "%s" "${value:-$2}"
}

prompt_free() {
  prompt_label "${RED_SOFT}" "$1"
  read_answer
}

confirm_default_yes() {
  local value
  prompt_label "${AMBER}" "$1" "$(tr_msg hint_yes_default)"
  value="$(read_answer)"
  [[ -z "$value" ]] || is_yes "$value"
}

confirm_default_no() {
  local value
  prompt_label "${AMBER}" "$1" "$(tr_msg hint_no_default)"
  value="$(read_answer)"
  is_yes "$value"
}

need_root() {
  [[ "${EUID}" -eq 0 ]] || die "$(tr_msg err_need_root)"
}

need_command() {
  command -v "$1" >/dev/null 2>&1 || die "$(tr_fmt err_missing_command "$1")"
}

is_installed() {
  [[ -x "$BINARY_PATH" ]]
}

# A name or a path typed by hand can carry a character the configuration
# format gives a meaning to, and a file that no longer parses is a server that
# no longer starts.
toml_string() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '"%s"' "$value"
}

# ── Checks ────────────────────────────────────────────────────────────────────

check_system() {
  section "$(tr_msg section_checks)"

  if [[ -f /etc/debian_version ]]; then
    success "$(tr_msg check_debian_ok)"
  else
    warn "$(tr_msg check_debian_warn)"
  fi

  if [[ -f /run/systemd/container ]] || grep -qa 'container=' /proc/1/environ 2>/dev/null; then
    info "$(tr_msg check_container)"
  fi

  local memory_mb
  memory_mb="$(awk '/MemTotal/ {printf "%d", $2/1024}' /proc/meminfo)"
  info "$(tr_fmt check_memory "$memory_mb")"
  if [[ "$memory_mb" -lt 6000 ]]; then
    warn "$(tr_msg warn_memory_low)"
  fi

  local disk_gb
  disk_gb="$(df -BG --output=avail / | tail -1 | tr -dc '0-9')"
  info "$(tr_fmt check_disk "$disk_gb")"
  if [[ "$disk_gb" -lt 20 ]]; then
    warn "$(tr_msg warn_disk_low)"
  fi

  if command -v ffmpeg >/dev/null 2>&1; then
    success "$(tr_fmt check_ffmpeg_ok "$(ffmpeg -version 2>/dev/null | head -1 | cut -d' ' -f1-3)")"
  else
    info "$(tr_msg check_ffmpeg_missing)"
  fi

  if [[ -d /dev/dri ]]; then
    success "$(tr_msg check_dri_ok)"
  else
    warn "$(tr_msg check_dri_missing)"
    panel "${AMBER}" "$(tr_msg dri_title)" \
      "$(tr_msg dri_line1)" \
      "$(tr_msg dri_line2)" \
      "" \
      "$(tr_msg dri_line3)" \
      "${BOLD}$(tr_msg dri_line4)${RESET}" \
      "" \
      "$(tr_msg dri_commands)" \
      "${CYAN}  pct set <CTID> --dev0 /dev/dri/renderD128${RESET}" \
      "${CYAN}  pct set <CTID> --dev1 /dev/dri/card0${RESET}" \
      "" \
      "$(tr_msg dri_replace)" \
      "$(tr_msg dri_later)"
  fi
}

check_port() {
  local port="$1"
  if command -v ss >/dev/null 2>&1 && ss -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]${port}\$"; then
    warn "$(tr_fmt check_port_busy "$port")"
    return 1
  fi
  success "$(tr_fmt check_port_free "$port")"
  return 0
}

# ── Install ───────────────────────────────────────────────────────────────────

install_packages() {
  section "$(tr_msg section_packages)"

  export DEBIAN_FRONTEND=noninteractive
  step "$(tr_msg step_apt_update)" apt-get update -qq

  step "$(tr_msg step_apt_install)" apt-get install -y --no-install-recommends \
    build-essential pkg-config git curl ca-certificates ffmpeg sqlite3

  if command -v cargo >/dev/null 2>&1; then
    success "$(tr_fmt rust_present "$(cargo --version)")"
  else
    step "$(tr_msg step_rust)" bash -c \
      "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal"
    export PATH="/root/.cargo/bin:${PATH}"
  fi
}

create_account_and_folders() {
  section "$(tr_msg section_account)"

  if id -u "$APP_USER" >/dev/null 2>&1; then
    success "$(tr_msg user_present)"
  else
    step "$(tr_msg step_user)" useradd --system --user-group --create-home \
      --home-dir "$DATA_DIR" --shell /usr/sbin/nologin "$APP_USER"
  fi

  step "$(tr_msg step_dirs)" bash -c "
    mkdir -p '$CONFIG_DIR' '$DATA_DIR' '$CACHE_DIR' '$BACKUP_DIR' '$SOURCE_DIR'
    chown -R '$APP_USER:$APP_GROUP' '$DATA_DIR' '$CACHE_DIR'
    chmod 750 '$DATA_DIR' '$CACHE_DIR'
  "
}

fetch_source() {
  section "$(tr_msg section_source)"

  if [[ -d "${SOURCE_DIR}/.git" ]]; then
    # An earlier install may have left a shallow copy behind, from before this
    # script always kept the full history. Completed once here rather than
    # left as it is: a shallow copy cannot show a diff against a commit it
    # never kept, and every update after this one depends on that history
    # still being there.
    if [[ "$(git -C "$SOURCE_DIR" rev-parse --is-shallow-repository 2>/dev/null)" == "true" ]]; then
      step "$(tr_msg step_unshallow)" git -C "$SOURCE_DIR" fetch --unshallow origin "$BRANCH"
    else
      step "$(tr_msg step_pull)" git -C "$SOURCE_DIR" fetch origin "$BRANCH"
    fi
    step "$(tr_msg step_pull)" git -C "$SOURCE_DIR" reset --hard "origin/${BRANCH}"
  else
    rm -rf "$SOURCE_DIR"
    # No --depth: the full history of the branch, kept from here on, is what
    # lets an update show what actually changed rather than only its result.
    step "$(tr_msg step_clone)" git clone --branch "$BRANCH" "$REPO_URL" "$SOURCE_DIR"
  fi
}

build_and_install() {
  section "$(tr_msg section_build)"
  info "$(tr_msg build_notice)"
  info "$(tr_msg build_nice)"

  export PATH="/root/.cargo/bin:${PATH}"
  # Low priority on both processor and disk, so a film playing right now keeps
  # its share of the machine.
  step "$(tr_msg step_build)" nice -n 15 cargo build --release --locked \
    --manifest-path "${SOURCE_DIR}/Cargo.toml"

  step "$(tr_msg step_install_binary)" bash -c "
    install -m 0755 '${SOURCE_DIR}/target/release/melyxar' '$BINARY_PATH'
  "
}

write_configuration() {
  section "$(tr_msg section_config)"

  if [[ -f "$CONFIG_FILE" ]]; then
    success "$(tr_msg config_present)"
    return 0
  fi

  local port
  port="$(prompt_default "$(tr_msg prompt_port)" "$DEFAULT_PORT")"
  check_port "$port" || true

  local tmp
  tmp="$(mktemp)"
  "$BINARY_PATH" print-default-config > "$tmp"
  sed -i "s/^port = .*/port = ${port}/" "$tmp"

  if confirm_default_yes "$(tr_msg prompt_add_library)"; then
    append_library "$tmp"
  fi

  step "$(tr_msg step_config)" bash -c "
    install -m 0640 -o root -g '$APP_GROUP' '$tmp' '$CONFIG_FILE'
    rm -f '$tmp'
  "
}

prompt_library_kind() {
  local kind
  while true; do
    kind="$(prompt_default "$(tr_msg prompt_library_kind)" "movies")"
    case "$kind" in
      movies | series | anime | shows | music )
        printf "%s" "$kind"
        return 0
        ;;
      * ) warn "$(tr_msg err_bad_kind)" >&2 ;;
    esac
  done
}

# A library is written only once it holds at least one folder: the server
# refuses a library that points nowhere, and discovering that when the service
# fails to start is too late to be of any use.
append_library() {
  local target="$1"
  local name kind path label
  local labels=() paths=()

  name="$(prompt_default "$(tr_msg prompt_library_name)" "Films")"
  kind="$(prompt_library_kind)"

  while true; do
    path="$(prompt_free "$(tr_msg prompt_root_path)")"
    [[ -z "$path" ]] && break

    if [[ "$path" != /* ]]; then
      warn "$(tr_msg root_not_absolute)"
      continue
    fi

    if [[ ! -d "$path" ]]; then
      confirm_default_no "$(tr_msg root_missing)" || continue
    elif ! runuser -u "$APP_USER" -- test -r "$path" 2>/dev/null; then
      warn "$(tr_msg root_unreadable)"
    fi

    label="$(prompt_default "$(tr_msg prompt_root_label)" "$(basename "$(dirname "$path")")")"
    labels+=("$label")
    paths+=("$path")
    success "$(tr_fmt root_added "$path")"
  done

  if [[ "${#paths[@]}" -eq 0 ]]; then
    warn "$(tr_msg library_without_root)"
    return 0
  fi

  {
    echo
    echo "[[libraries]]"
    echo "name = $(toml_string "$name")"
    echo "kind = $(toml_string "$kind")"
    echo "metadata_language = $(toml_string "$APP_LANG")"
    local index
    for index in "${!paths[@]}"; do
      echo
      echo "[[libraries.roots]]"
      echo "label = $(toml_string "${labels[index]}")"
      echo "path = $(toml_string "${paths[index]}")"
    done
  } >> "$target"
}

install_service() {
  section "$(tr_msg section_service)"

  local tmp
  tmp="$(mktemp)"
  cat > "$tmp" <<UNIT
[Unit]
Description=Melyxar media server
Documentation=https://github.com/Victor-root/melyxar
After=network-online.target
Wants=network-online.target

[Service]
Type=exec
User=${APP_USER}
Group=${APP_GROUP}
ExecStart=${BINARY_PATH} --config ${CONFIG_FILE} serve
Restart=on-failure
RestartSec=5

# Playback sessions own external processes. Stopping has to be given time to
# close them, otherwise they are left behind, which is the failure this whole
# project set out to avoid.
KillSignal=SIGTERM
TimeoutStopSec=30
KillMode=mixed

# The server needs nothing beyond its own folders and the media it is given.
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=${DATA_DIR} ${CACHE_DIR}

[Install]
WantedBy=multi-user.target
UNIT

  step "$(tr_msg step_unit)" bash -c "
    install -m 0644 '$tmp' '$UNIT_FILE'
    rm -f '$tmp'
    systemctl daemon-reload
  "
  step "$(tr_msg step_enable)" systemctl enable "$SERVICE"
  step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
}

show_done() {
  local port address
  port="$(awk -F' *= *' '/^port/ {print $2; exit}' "$CONFIG_FILE" 2>/dev/null || echo "$DEFAULT_PORT")"
  address="$(hostname -I 2>/dev/null | awk '{print $1}')"
  [[ -z "$address" ]] && address="127.0.0.1"

  section "$(tr_msg section_done)"
  panel "${GREEN}" "$(tr_msg done_title)" \
    "$(tr_msg done_open)" \
    "${BOLD}${CYAN}  http://${address}:${port}${RESET}" \
    "" \
    "$(tr_msg done_config)" \
    "${GRAY}  ${CONFIG_FILE}${RESET}" \
    "" \
    "$(tr_msg done_logs)" \
    "${GRAY}  journalctl -u ${SERVICE} -f${RESET}" \
    "" \
    "$(tr_msg done_report)" \
    "${GRAY}  melyxar --config ${CONFIG_FILE} doctor${RESET}" \
    "" \
    "$(tr_msg done_again)" \
    "${GRAY}  bash <(curl -fsSL ${SCRIPT_URL})${RESET}"
}

action_install() {
  check_system
  install_packages
  create_account_and_folders
  fetch_source
  build_and_install
  write_configuration
  install_service
  show_done
}

# What changed between the commit that was running and the one about to
# replace it: the file list first, in git's own shape, then one sentence with
# the totals in it. Nothing here needs the branch's history to reach back to
# any particular point, only that both commits are still present, which a full
# clone guarantees and a shallow one used not to.
show_update_diff() {
  local before="$1"
  local after="$2"

  # Nothing to compare against: the source was not there before this run, so
  # what is about to be built is not a change from anything, it is the first
  # copy of it.
  [[ -z "$before" ]] && return 0

  section "$(tr_msg section_update_diff)"

  if [[ "$before" == "$after" ]]; then
    info "$(tr_msg update_diff_up_to_date)"
    return 0
  fi

  local color_stat=()
  [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]] && color_stat=(--color=always)

  # git's own table of files and bars needs no translation, being names and
  # graphics rather than words. Only its last line, the English summary
  # sentence, is dropped, and replaced below with a translated one built from
  # the same numbers the line-counting menu entry already knows how to add up.
  git -C "$SOURCE_DIR" diff --stat "${color_stat[@]}" "$before" "$after" |
    sed -e '$d' -e 's/^/  /'

  local moved=()
  read -r -a moved < <(
    git -C "$SOURCE_DIR" diff --numstat "$before" "$after" |
      awk '$1 != "-" { added += $1; removed += $2; files += 1 }
           END { printf "%d %d %d\n", added + 0, removed + 0, files + 0 }'
  )
  echo
  success "$(tr_fmt lines_changed "${moved[@]}")"
}

action_update() {
  is_installed || die "$(tr_msg err_not_installed)"
  # A backup before any change, so a failed update is never a lost library.
  action_backup quiet

  local before
  before="$(git -C "$SOURCE_DIR" rev-parse HEAD 2>/dev/null || echo "")"
  fetch_source
  local after
  after="$(git -C "$SOURCE_DIR" rev-parse HEAD)"
  show_update_diff "$before" "$after"

  build_and_install
  step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
  show_done
}

action_status() {
  is_installed || die "$(tr_msg err_not_installed)"
  echo
  "$BINARY_PATH" --config "$CONFIG_FILE" doctor 2>/dev/null
}

# What the branch being deployed is measured against.
COUNTED_AGAINST="main"

# What is written by hand rather than produced. The built interface, the lock
# files and the pictures are large enough to drown everything else, and a count
# they take part in says nothing about the work.
LINES_LEFT_OUT=(
  ':!web/dist'
  ':!*.lock'
  ':!*.png'
  ':!*.webp'
  ':!*.svg'
  ':!*.ico'
)

# How many lines one branch is made of.
lines_written_on() {
  local ref="$1"
  # One line per file, ending in its count. A path can hold a colon, so the
  # count is read from the end rather than from a field number.
  git -C "$SOURCE_DIR" grep -I -c '^' "$ref" -- "${LINES_LEFT_OUT[@]}" |
    awk -F: '{ total += $NF } END { print total + 0 }'
}

action_lines() {
  fetch_source

  if [[ "$BRANCH" == "$COUNTED_AGAINST" ]]; then
    section "$(tr_msg section_lines)"
    info "$(tr_fmt lines_on "$BRANCH" "$(lines_written_on "origin/${BRANCH}")")"
    info "$(tr_msg lines_counted)"
    return 0
  fi

  # The branch is cloned on its own, so the one it is compared with has to be
  # asked for by name and put where a comparison can find it.
  step "$(tr_fmt step_fetch_main "$COUNTED_AGAINST")" \
    git -C "$SOURCE_DIR" fetch --depth 1 origin \
    "+refs/heads/${COUNTED_AGAINST}:refs/remotes/origin/${COUNTED_AGAINST}"

  section "$(tr_msg section_lines)"

  local here there
  there="$(lines_written_on "origin/${COUNTED_AGAINST}")"
  here="$(lines_written_on "origin/${BRANCH}")"

  info "$(tr_fmt lines_on "$COUNTED_AGAINST" "$there")"
  info "$(tr_fmt lines_on "$BRANCH" "$here")"

  if ((here > there)); then
    success "$(tr_fmt lines_ahead "$BRANCH" "$((here - there))" "$COUNTED_AGAINST")"
  elif ((here < there)); then
    success "$(tr_fmt lines_behind "$BRANCH" "$((there - here))" "$COUNTED_AGAINST")"
  else
    success "$(tr_fmt lines_same "$BRANCH" "$COUNTED_AGAINST")"
  fi

  # What moved, which is a different question from how long each side is: a
  # line rewritten counts on both sides and changes neither total.
  local moved=()
  read -r -a moved < <(
    git -C "$SOURCE_DIR" diff --numstat \
      "origin/${COUNTED_AGAINST}" "origin/${BRANCH}" -- "${LINES_LEFT_OUT[@]}" |
      awk '$1 != "-" { added += $1; removed += $2; files += 1 }
           END { printf "%d %d %d\n", added + 0, removed + 0, files + 0 }'
  )
  info "$(tr_fmt lines_changed "${moved[@]}")"
  info "$(tr_msg lines_counted)"
}

action_backup() {
  local quiet="${1:-}"
  [[ "$quiet" != "quiet" ]] && section "$(tr_msg section_backup)"

  local database="${DATA_DIR}/melyxar.db"
  if [[ ! -f "$database" ]]; then
    warn "$(tr_msg backup_none)"
    return 0
  fi

  local stamp destination
  stamp="$(date +%Y%m%d-%H%M%S)"
  destination="${BACKUP_DIR}/melyxar-${stamp}.db"

  # Only the engine's own copy is consistent while the server writes, and a
  # plain copy of a database in write ahead mode leaves the recent changes
  # behind, so a missing tool stops the backup rather than producing one that
  # cannot be trusted.
  need_command sqlite3

  step "$(tr_msg step_backup)" bash -c "
    mkdir -p '$BACKUP_DIR'
    sqlite3 '$database' \".backup '$destination'\"
    chown '$APP_USER:$APP_GROUP' '$destination'
    ls -1t '$BACKUP_DIR'/melyxar-*.db 2>/dev/null | tail -n +$((BACKUP_KEEP + 1)) | xargs -r rm -f
  "
  success "$(tr_fmt backup_done "$destination")"
}

action_restore() {
  is_installed || die "$(tr_msg err_not_installed)"
  section "$(tr_msg section_restore)"

  local backups=() backup
  mapfile -t backups < <(ls -1t "${BACKUP_DIR}"/melyxar-*.db 2>/dev/null || true)
  if [[ "${#backups[@]}" -eq 0 ]]; then
    warn "$(tr_fmt restore_none "$BACKUP_DIR")"
    return 0
  fi

  echo
  local index=1
  for backup in "${backups[@]}"; do
    printf "  %b%2d%b) %s %b(%s)%b\n" "${BOLD}" "$index" "${RESET}" \
      "$(basename "$backup")" "${DIM}" "$(date -r "$backup" '+%Y-%m-%d %H:%M')" "${RESET}"
    index=$((index + 1))
  done
  echo

  local choice
  choice="$(prompt_default "$(tr_msg restore_pick)" "1")"
  # A line that is not a number must not become a position in the list, where
  # a negative one would quietly pick the oldest backup of them all.
  [[ "$choice" =~ ^[1-9][0-9]*$ ]] || die "$(tr_fmt err_bad_choice "$choice")"
  local picked="${backups[$((choice - 1))]:-}"
  [[ -z "$picked" ]] && die "$(tr_fmt err_bad_choice "$choice")"

  confirm_default_no "$(tr_msg restore_confirm)" || { info "$(tr_msg cancelled)"; return 0; }

  local kept
  kept="${BACKUP_DIR}/before-restore-$(date +%Y%m%d-%H%M%S).db"
  step "$(tr_msg step_stop)" systemctl stop "$SERVICE"
  step "$(tr_msg step_restore)" bash -c "
    if [[ -f '${DATA_DIR}/melyxar.db' ]]; then
      mv '${DATA_DIR}/melyxar.db' '$kept'
    fi
    rm -f '${DATA_DIR}/melyxar.db-wal' '${DATA_DIR}/melyxar.db-shm'
    cp -a '$picked' '${DATA_DIR}/melyxar.db'
    chown '$APP_USER:$APP_GROUP' '${DATA_DIR}/melyxar.db'
  "
  step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
  if [[ -f "$kept" ]]; then
    success "$(tr_fmt restore_done "$kept")"
  else
    success "$(tr_msg restore_done_fresh)"
  fi
}

action_uninstall() {
  section "$(tr_msg section_uninstall)"

  confirm_default_no "$(tr_msg uninstall_confirm)" || { info "$(tr_msg cancelled)"; return 0; }

  if systemctl list-unit-files 2>/dev/null | grep -q "^${SERVICE}.service"; then
    step "$(tr_msg step_stop)" bash -c "
      systemctl disable --now '$SERVICE' || true
      rm -f '$UNIT_FILE'
      systemctl daemon-reload
    "
  fi

  rm -f "$BINARY_PATH"
  rm -rf "$SOURCE_DIR"

  if confirm_default_no "$(tr_msg uninstall_keep_data)"; then
    rm -rf "$DATA_DIR" "$CACHE_DIR" "$CONFIG_DIR"
    userdel "$APP_USER" 2>/dev/null || true
  else
    info "$(tr_fmt uninstall_data_kept "$DATA_DIR" "$CONFIG_DIR")"
  fi

  success "$(tr_msg uninstall_done)"
}

# ── Menu ──────────────────────────────────────────────────────────────────────

menu() {
  echo
  printf "%b%s%b\n" "${BOLD}" "$(tr_msg menu_title)" "${RESET}"
  echo
  printf "   %b1%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_install)"
  printf "   %b2%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_update)"
  printf "   %b3%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_status)"
  printf "   %b4%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_backup)"
  printf "   %b5%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_restore)"
  printf "   %b6%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_uninstall)"
  printf "   %b7%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_lines)"
  printf "   %b0%b) %s\n" "${BOLD}${GRAY}" "${RESET}" "$(tr_msg menu_quit)"
  echo

  local choice
  choice="$(prompt_default "$(tr_msg prompt_choice)" "1")"

  case "$choice" in
    1) action_install ;;
    2) action_update ;;
    3) action_status ;;
    4) action_backup ;;
    5) action_restore ;;
    6) action_uninstall ;;
    7) action_lines ;;
    0) exit 0 ;;
    *) die "$(tr_fmt err_bad_choice "$choice")" ;;
  esac
}

main() {
  need_root
  need_command awk
  need_command sed

  # Switches are read first and taken out, so the action stays the first word
  # whether or not one was given.
  local rest=()
  while (($#)); do
    case "$1" in
      -v | --verbose) VERBOSE=1 ;;
      -q | --quiet) VERBOSE=0 ;;
      *) rest+=("$1") ;;
    esac
    shift
  done
  set -- ${rest[@]+"${rest[@]}"}

  banner
  [[ "$VERBOSE" -eq 1 ]] && info "$(tr_msg verbose_on)"

  # A named action runs straight away, which is what the update entry in a
  # scheduled job needs.
  case "${1:-}" in
    install)   action_install ;;
    update)    action_update ;;
    status)    action_status ;;
    backup)    action_backup ;;
    restore)   action_restore ;;
    uninstall) action_uninstall ;;
    lines)     action_lines ;;
    # Without a terminal there is nobody to answer the menu, and an install is
    # far too heavy a thing to start on a default nobody chose.
    "")        has_terminal || die "$(tr_msg err_no_terminal)"; menu ;;
    *)         die "$(tr_fmt err_bad_choice "$1")" ;;
  esac
}

main "$@"
