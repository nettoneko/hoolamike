set working-directory := './playground'
set positional-arguments

default:
    just --list

test-empty:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            --skip-kind from-archive \
            --skip-kind patched-from-archive \
            --skip-kind transformed-texture \
            --skip-kind remapped-inline-file \
            --skip-kind inline-file \
            --skip-kind create-bsa

@test-all +args:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            {{args}}


@test-from-archive:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            --skip-kind patched-from-archive \
            --skip-kind transformed-texture \
            --skip-kind remapped-inline-file \
            --skip-kind inline-file \
            --skip-kind create-bsa \
            "$@"

@test-create-bsa +args:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            --skip-kind from-archive \
            --skip-kind patched-from-archive \
            --skip-kind transformed-texture \
            --skip-kind remapped-inline-file \
            --skip-kind inline-file \
            {{args}}
test-transformed-texture +args:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            --skip-kind from-archive \
            --skip-kind patched-from-archive \
            --skip-kind remapped-inline-file \
            --skip-kind inline-file \
            --skip-kind create-bsa \
            {{args}}

test-remapped-inline-file:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            --skip-kind from-archive \
            --skip-kind patched-from-archive \
            --skip-kind transformed-texture \
            --skip-kind inline-file \
            --skip-kind create-bsa
