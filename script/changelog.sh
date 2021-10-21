#!/usr/bin/env bash

set -uo pipefail

CLEAR='\033[0m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'

bail() {
  MESSAGE="$@"
  echo -e "${RED}failed: ${MESSAGE}${CLEAR}" >/dev/stderr
  exit 1
}

# cd to git repository root
GIT_ROOT=$(git rev-parse --show-toplevel || bail "not in git repository")

usage() {
  if [ -n "${1:-}" ]; then
    echo -e "${RED}👉 $1${CLEAR}\n";
  fi
  echo "
$0: github_changelog_generator + chandler

  This script is designed for a combination of auto-generated and manually
  drafted release notes. It combines:
    - \`github_changelog_generator\` (gem install github_changelog_generator)
      to generate release notes from GitHub issues and pull requests
    - Supporting manually editing your CHANGELOG.md by using GCG with the
      --since-tag option and concatenating new entries onto your old
      CHANGELOG.md (no need to use a tagged GH issue to insert a simple comment
      into the release notes)
    - \`chandler\` (gem install chandler) to keep your CHANGELOG.md and GitHub
      releases in sync

Usage: $0 subcommand [args]

  (Common args):
    --tag vX.X.X      What tag to list unreleased entries under
    --remote          Which git remote to use (default \"origin\", assumes it has a GitHub URL and gets repo owner/name from it)
    --help            Show usage
    --config CONFIG   Use a predefined configuration (wasm, citeproc, ffi).
    --area AREA       Use A-AREA to filter PRs/Issues by label. Also --areas. Impliedly includes 'core'.
    --name NAME       Set the heading to '# Changelog (NAME)'
    --prefix PREFIX   Use 'PREFIX-' as a tag prefix, e.g. 'wasm-' for wasm-vX.X.X tags

  changelog [args]

    Generates a changelog by concatenating:
      - a base (default: --base CHANGELOG.md)
      - all unreleased GitHub issues/PRs under a new heading specified with '--tag TAG'
      - and writing back to CHANGELOG.md

    --base BASE.md    Write new changelog entries on top of an existing changelog (default: CHANGELOG.md)
    --since TAG       Don't use 'git describe' to find the most recent tagged version, use this tag instead
    --output OUT      Set output file (default is to write back into CHANGELOG.md)

  release [args]

    Commits, tags, pushes, syncs.
        [ 0. assumes you have just drafted a new CHANGELOG.md, uncommitted ]
        1. commits the resulting CHANGELOG.md with a '[skip ci] release TAG' message
        2. creates an annotated tag (TAG) on the resulting commit
        3. pushes the resulting tag to the remote, and pushes the current branch
        4. uses \`chandler push\` to sync the repo's tags + changelog contents to GitHub Releases

    -i | --interactive  Generate a changelog first, then edit it with \$EDITOR, then continue
    --grip              Preview the interactive-edited DRAFT_CHANGELOG.md with \`grip\` (brew install grip)
    --yes               Skip all confirmation prompts (potentially destructive)
"
  if [ -n "${1:-}" ]; then
    echo -e "${RED}👉 $1${CLEAR}\n";
  fi
  exit 0
}

AUTOCONFIRM=false
CONFIRM_COLOUR="${CYAN}"
info() {
  echo "==> $@" >/dev/stderr
}
warning() {
  echo -e "${YELLOW}==> $@${CLEAR}" >/dev/stderr
}
confirm() {
  local MSG="==> $1"
  local BAIL="${2:-}"
  local REPLY
  BAIL=$(if [[ "$BAIL" == "--bail" ]]; then echo "true"; else echo "false"; fi)
  if $AUTOCONFIRM; then
    echo -e "${CONFIRM_COLOUR}$MSG${CLEAR} [yes]"
    return 0
  fi
  askit() {
    echo -n -e "${CONFIRM_COLOUR}$MSG${CLEAR}"
    if $BAIL; then read -p " [y/n/?] "; else read -p " [y/n/q/?] "; fi
  }
  askit
  if $BAIL; then
    while [[ ! $REPLY =~ ^[YyNnQq]$ ]]; do
      echo -e "type one of y (yes) or n/q (quit)"
      askit
    done
  else
    while [[ ! $REPLY =~ ^[YySsNnQq]$ ]]; do
      echo -e "type only y (yes), n/s (skip) or q (quit)"
      askit
    done
  fi
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    if $BAIL || [[ $REPLY =~ ^[Qq]$ ]]; then bail "cancelled"; fi
    return 1
  fi
}
warn_confirm() {
  CONFIRM_COLOUR="${YELLOW}" confirm "$1" "${2:-}"
  return $?
}

# https://stackoverflow.com/a/49880990
netrc-fetch () {
  < $HOME/.netrc awk -v host=$1 -v field=${2:-password} '
    {
      for (i=1; i <= NF; i += 2) {
        j=i+1;
        if ($i == "machine") {
          found = ($j == host);
        } else if (found && ($i == field)) {
          print $j;
          exit;
        }
      }
    }
  '
}

needs() {
  if ! command -v "$1" &>/dev/null; then
    bail "$1 not installed ${2:-}"
  fi
}

if [ -z "${GITHUB_TOKEN:-}" ]; then
  # GH_USER=$(netrc-fetch api.github.com login)
  # this must be a personal access token, login irrelevant
  GITHUB_TOKEN=$(netrc-fetch api.github.com password)
fi
if [ -z "${GITHUB_TOKEN:-}" ]; then
  bail "no GITHUB_TOKEN env variable or netrc entry for api.github.com found"
fi

# this is for accessibility. The defaults are **bolded** but should probably be <h4>.
# --summary-label ""
HEADER_4_LABELS=( \
  --breaking-label     "#### Breaking changes:" \
  --enhancement-label  "#### Implemented enhancements:" \
  --bugs-label         "#### Fixed bugs:" \
  --deprecated-label   "#### Deprecated:" \
  --removed-label      "#### Removed:" \
  --security-label     "#### Security fixes:" \
  --issues-label       "#### Closed issues:" \
  --pr-label           "#### Merged pull requests:" \
  --unreleased-label   "Unreleased:" )

wrapper () {
  local argv=( "$@" )
  local GIT_REMOTE="origin"
  local GH_OWNER=""
  local GH_REPO=""

  local GRIP=false

  # args to pass to github_changelog_generator
  local FUTURE_RELEASE=()
  local INSERT_TAG="Unreleased"
  local SINCE_TAG=()
  local SINCE_TAG_MANUAL=false

  since_tag_auto() {
    # $1 is a glob, mustn't be a regex
    local GLOB="$1"
    if ! git describe --match "$GLOB" --abbrev=0 &>/dev/null; then
      confirm "no tags in repo, \`git describe --match \"$GLOB\" --abbrev=0\` found nothing.
    Continue by creating first ever release?:" --bail
    else
      SINCE_TAG=(--since-tag $(git describe --match "$GLOB" --abbrev=0))
    fi
  }

  local REST=()
  local OUTPUT="CHANGELOG.md"
  local BASE="CHANGELOG.md"
  local AREA=( )
  local V_PREFIX=""
  local V_PREFIX_REGEX="^v\d"
  local V_GLOB="v[0-9]*"
  local INCLUDE_TAGS_REGEX=( --include-tags-regex "$V_PREFIX_REGEX" )
  local INCLUDE_LABELS=()
  local HEADER=""

  # previewing
  local FUTURE=false
  local EXISTING=false
  local RAW=false
  local NO_FETCH=false
  local NO_UNRELEASED=false
  local INTERNAL=""
  local YES=false
  local EDIT=false
  local GENERATE=false

  local UNRELEASED_ARG=()
  local INSERT_TAG_REGEX=""

  parse_params() {
    local SUB="$1"
    shift
    local argv=( "$@" )

    parse_common () {
      local argv=( "$@" )
      case "$1" in
        --since-tag) SINCE_TAG_MANUAL=true; SINCE_TAG=(--since-tag "$2"); return 2;;
        --since-commit) info "$2" + "${argv[@]}"; SINCE_TAG=(--release-branch master --since-commit "$2"); return 2;;
        --existing)
          EXISTING=true; FUTURE_TAG=""; NO_UNRELEASED=true; EXISTING_TAG="$2"; return 2;;
        --tag)
          FUTURE=true; NO_UNRELEASED=false;
          EXISTING_TAG=""; FUTURE_TAG="$2"; return 2;;
        --remote) GIT_REMOTE="$2"; return 2;;
        --area|--areas)
          IFS=',' read -r -a AREA <<< "$2";
          return 2;;
        --name) HEADER="# Changelog ($2)"; return 2;;
        --prefix) V_PREFIX="$2"; V_PREFIX_REGEX="^$2-"; V_GLOB="$V_PREFIX-v[0-9]*"; return 2;;
        --config|--configuration|-c)
          local name=""; local area_name=""; local prefix=""; local regex=""; local glob="";
          case "$2" in
            citeproc) cd "$GIT_ROOT/crates/citeproc";
              name="crates/citeproc"; area_name="crates/citeproc"; prefix=""; regex="^v\d"; glob="v[0-9]*";;
            wasm) cd "$GIT_ROOT/crates/wasm";
              name="@citeproc-rs/wasm"; area_name="wasm"; prefix="wasm-"; regex="^wasm-v\d"; glob="wasm-v[0-9]*";;
            ffi) cd "$GIT_ROOT/bindings/ffi";
              name="ffi"; area_name="ffi"; prefix="ffi-"; regex="^ffi-v\d"; glob="ffi-v[0-9]*";;
            *) usage "unknown -c configuration \"$2\"";;
          esac;
          HEADER="# Changelog ($name)";
          AREA=( "$area_name" );
          V_PREFIX="$prefix";
          V_PREFIX_REGEX="$regex";
          V_GLOB="$glob"
          return 2;;
        --) len=$#; shift; REST=("$@"); return $len;;
        --help|-h) usage;;
        *) usage "$SUB: Unknown parameter passed: $1";;
      esac;
    }

    parse_changelog() {
      GENERATE=true
      OUTPUT="CHANGELOG.md"
      while [[ "$#" > 0 ]]; do
        local argv=( "$@" )
        case "$1" in
          --raw) RAW=true; shift;;
          --base) BASE="$2"; shift 2;;
          --output) OUTPUT="$2"; shift;;
          *) parse_common "${argv[@]}"; shift $?;;
        esac
      done
    }

    # unused, for if you want to build a tool to create a github release
    parse_release () {
      OUTPUT="CHANGELOG.md"
      while [[ "$#" > 0 ]]; do 
        local argv=( "$@" )
        case "$1" in
          --internal) INTERNAL="$2"; shift 2;;
          # --generate|-g) GENERATE=true; shift;;
          -i|--interactive) EDIT=true; GENERATE=true; shift;;
          --grip) GRIP=true; needs grip "(brew install grip)"; shift;;
          --yes|-y) YES=true; shift;;
          *) parse_common "${argv[@]}"; shift $?;;
        esac
      done
    }

    if [[ "$SUB" == "changelog" ]]; then
      parse_changelog "${argv[@]}"
    elif [[ "$SUB" == "release" ]]; then
      parse_release "${argv[@]}"
    elif [[ "$SUB" == "sync" ]]; then
      parse_release "${argv[@]}"
    else
      usage "unrecognised subcommand: $SUB"
    fi

    REMOTE_URL=$(git config --get remote.$GIT_REMOTE.url || bail "could not find git remote $GIT_REMOTE")
    GH_OWNER=$(basename $(dirname $REMOTE_URL))
    GH_REPO=$(basename $REMOTE_URL .git)

    if [[ "$SUB" == "release" ]] && ! $FUTURE && ! $EXISTING; then
      usage "Must pass --tag to subcommand release"
    fi
    if $FUTURE && $EXISTING; then
      usage "can't specify --tag/--future-release and --existing together"
    fi
    if $FUTURE; then
      FUTURE_TAG="$V_PREFIX${FUTURE_TAG#$V_PREFIX}"
      FUTURE_RELEASE=(--future-release "$FUTURE_TAG")
    fi
    INCLUDE_TAGS_REGEX=(--include-tags-regex "$V_PREFIX_REGEX")

    if ! $SINCE_TAG_MANUAL; then
      since_tag_auto "$V_GLOB"
    fi

    if $FUTURE; then
      INSERT_TAG="$FUTURE_TAG"
    elif $EXISTING; then
      INSERT_TAG="$EXISTING_TAG"
    fi

    case "$INTERNAL" in
      check) return 1 ;;
      remote) echo "$GIT_REMOTE"; return 1 ;;
      repo) echo "$GH_OWNER/$GH_REPO"; return 1 ;;
      yes) echo $YES; return 1 ;;
      tag)
        if ! $FUTURE; then
          bail "cannot use release without --tag <tag>"
        fi
        echo "$FUTURE_TAG"
        return 1
        ;;
      path) # return the path to the changelog path
        echo "$(pwd)/$OUTPUT"
        return 1
        ;;
      prefix) # return the path to the changelog path
        echo "$V_PREFIX"
        return 1
        ;;
    esac

    if $NO_FETCH && ! $EXISTING && [ "$BASE" = "$OUTPUT" ]; then
      # noop
      return 0
    fi

    INSERT_TAG_REGEX=$(printf "$INSERT_TAG" | sed 's/\./\\\\./g')
    if $NO_UNRELEASED; then UNRELEASED_ARG=(--no-unreleased); fi

    if ! [ "${#AREA[@]}" -eq 0 ] && [ -z "$HEADER" ]; then
      HEADER="# Changelog (${AREA[0]})"
    fi

    AREA+=( core )
  }

  set +e
  parse_params "${argv[@]}"
  if [ $? = 1 ]; then
    return 0
  fi

  local TMP=$(mktemp -d)
  trap "rm -rf -- $TMP" EXIT
  local TMP_OUT="$TMP/CHANGELOG.md"

  # execute_awk () {
  #   set +e
  #   awk \
  #     -v insert_tag="$INSERT_TAG_REGEX" \
  #     -v draft="$DRAFT" \
  #     -f ./script/append_draft.awk \
  #     "$TMP_OUT" > "$TMP/CHANGELOG.md.out"
  #
  #   if [[ $? -ne 0 ]]; then
  #     bail "failed to insert draft release text into version $INSERT_TAG (version not present in the changelog)"
  #   fi
  #
  #   mv "$TMP/CHANGELOG.md.out" "$OUTPUT"
  # }

  execute_gen () {
    BUG_LABELS=I-schema,I-spec,I-bug,I-packaging,I-build
    AREA_LABELS=A-ci,A-docs
    if ! [ -z "$AREA" ]; then
      # for --issue-line-labels: which labels to show in the markdown
      AREA_LABELS=$AREA_LABELS,A-core
      function join_by { local d=${1-} f=${2-}; if shift 2; then printf %s "$f" "${@/#/$d}"; fi; }
      # which issues/PRs to include at all
      info "areas: ${AREA[@]}"
      local AREAS_PREFIXED=$(join_by ',' "${AREA[@]/#/A-}")
      INCLUDE_LABELS=(--include-labels "$AREAS_PREFIXED")
    fi
    if ! $NO_FETCH; then
      github_changelog_generator -u $GH_OWNER -p $GH_REPO -t $GITHUB_TOKEN \
        "${FUTURE_RELEASE[@]}" \
        "${UNRELEASED_ARG[@]}" \
        "${SINCE_TAG[@]}" \
        --base "$BASE" \
        --output "$TMP_OUT" \
        --header-label "$HEADER" \
        "${INCLUDE_TAGS_REGEX[@]}" \
        "${HEADER_4_LABELS[@]}" \
        "${INCLUDE_LABELS[@]}" \
        --issue-line-labels $AREA_LABELS,$BUG_LABELS \
        --bug-labels $BUG_LABELS \
        --no-author \
        "${REST[@]}"
    else
      cp "$BASE" "$TMP_OUT"
    fi

    # execute_awk
  }


  if $GENERATE; then
    if $EDIT; then
      if [ -f DRAFT_CHANGELOG.md ]; then
        if warn_confirm "DRAFT_CHANGELOG.md already exists. Delete it and recreate? (Otherwise edit and release from DRAFT_CHANGELOG.md)"; then
          rm DRAFT_CHANGELOG.md
          execute_gen
          mv "$TMP_OUT" DRAFT_CHANGELOG.md
        fi
        # else noop
      else
        execute_gen
        mv "$TMP_OUT" DRAFT_CHANGELOG.md
      fi
      sizeof () { wc -c DRAFT_CHANGELOG.md | awk '{ print $1 }'; }
      local SIZE=$(sizeof)
      if $GRIP; then
        needs grip "(brew install grip)"
        grip -b DRAFT_CHANGELOG.md --pass=$GITHUB_TOKEN &>/dev/null & $EDITOR DRAFT_CHANGELOG.md && kill $!
      else
        $EDITOR DRAFT_CHANGELOG.md
      fi
      if ! [ -s DRAFT_CHANGELOG.md ] || ! grep -q '[^[:space:]]' < DRAFT_CHANGELOG.md; then
        # file is empty (ignoring whitespace)
        bail "Edited CHANGELOG.md is empty, bailing out"
      elif [[ "$(sizeof)" -lt "$SIZE" ]]; then
        warn_confirm "The edited CHANGELOG.md is smaller than it was before. Are you sure you want to continue releasing?" --bail
      fi
      mv DRAFT_CHANGELOG.md "$OUTPUT"

    elif $RAW; then
      github_changelog_generator -u $GH_OWNER -p $GH_REPO -t $GITHUB_TOKEN "${REST[@]}"
    else
      execute_gen
      mv "$TMP_OUT" "$OUTPUT"
    fi
  fi
}

SUB="$1"
shift
argv=( "$@" )
case $SUB in
  --help|-h) usage ;;

  changelog)
    needs github_changelog_generator "(gem install github_changelog_generator)"
    wrapper changelog "${argv[@]}"
    ;;

  release)
    needs chandler "(gem install chandler)"
    wrapper release --internal check "${argv[@]}"
    TAG=$(wrapper release --internal tag "${argv[@]}")
    AUTOCONFIRM=$(wrapper release --internal yes "${argv[@]}")
    GIT_REMOTE=$(wrapper release --internal remote "${argv[@]}")
    GH_OWNER_REPO=$(wrapper release --internal repo "${argv[@]}")
    # write the changelog
    # TODO: since_tag is wrong
    wrapper release "${argv[@]}"
    info "working directory: $PWD"
    exit

    if confirm "add CHANGELOG.md and create release commit? (if not, just tags current commit)"; then
      git add CHANGELOG.md

      git diff --stat HEAD
      if git status --porcelain --untracked-files=no | grep -v '^M  CHANGELOG.md$' &>/dev/null; then
        warn_confirm "git repository has other changes. continue?" --bail
      fi

      # doesn't create a commit if there are no changes at all, assumes you're re-trying
      # you can't very well
      if git diff-index --quiet HEAD CHANGELOG.md; then
        CONFIRM_COLOUR="$YELLOW" confirm "warning: there were no changes to CHANGELOG.md. you may end up with an empty release. continue anyway?" --bail
      fi
      git commit -m "[skip ci] release $TAG"
    else
      info "skipped changelog commit, HEAD is at $(git rev-parse HEAD)"
      if ! git diff-index --quiet HEAD CHANGELOG.md; then
        git status -sb
        warn_confirm "warning: CHANGELOG.md has uncommitted changes. continue anyway?" --bail
      fi
    fi

    FORCE_PUSH_TAG=""
    mktag() { set -e; git tag -a -m "release $TAG" $TAG; }
    if git rev-parse $TAG &>/dev/null; then
      if [[ "$(git rev-parse $TAG)" == "$(git rev-parse HEAD)" ]]; then
        info "using existing tag $TAG ($(git rev-parse $TAG)) == HEAD"
      else
        warning "warning: tag exists: $TAG ($(git rev-parse $TAG))"
        warn_confirm "delete, recreate, and force push existing tag $TAG?" && git tag -d "$TAG" && mktag && FORCE_PUSH_TAG="--force"
      fi
    else
      info "creating git tag $TAG from current HEAD"
      mktag
    fi

    info "pushing git tag $TAG to remote $GIT_REMOTE"
    git push $FORCE_PUSH_TAG $GIT_REMOTE $TAG

    info "pushing this branch to remote $GIT_REMOTE"
    git push origin

    # just push all of them so any changelog edits you make are synced. it doesn't take that long.
    if confirm "sync CHANGELOG.md to GitHub Releases?"; then
      env CHANDLER_GITHUB_API_TOKEN=$GITHUB_TOKEN chandler push --github=${CHANDLER_TARGET:-$GH_OWNER_REPO}
    else
      info "skipped chandler push"
    fi
    ;;

  sync)
    needs chandler "(gem install chandler)"
    wrapper sync --internal check "${argv[@]}"
    TAG=$(wrapper sync --internal tag "${argv[@]}")
    AUTOCONFIRM=$(wrapper sync --internal yes "${argv[@]}")
    GIT_REMOTE=$(wrapper sync --internal remote "${argv[@]}")
    GH_OWNER_REPO=$(wrapper sync --internal repo "${argv[@]}")
    OUTPUT_PATH="$(wrapper sync --internal path "${argv[@]}")"
    CH_PREFIX="$(wrapper sync --internal prefix "${argv[@]}")"
    cd "$GIT_ROOT"
    info "syncing  $OUTPUT_PATH"

    # just push all of them so any changelog edits you make are synced. it doesn't take that long.
    confirm "sync CHANGELOG.md to GitHub Releases (dry run)?" --bail
    env CHANDLER_GITHUB_API_TOKEN=$GITHUB_TOKEN chandler push --dry-run --tag-prefix=${CH_PREFIX} --changelog=${OUTPUT_PATH} --github=${CHANDLER_TARGET:-$GH_OWNER_REPO}
    ;;
  *) usage "unrecognised subcommand $SUB";;
esac
