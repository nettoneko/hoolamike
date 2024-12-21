set working-directory := './playground'
default:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads
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

test-create-bsa:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            --skip-kind from-archive \
            --skip-kind patched-from-archive \
            --skip-kind transformed-texture \
            --skip-kind remapped-inline-file \
            --skip-kind inline-file
test-transformed-texture:
    cargo \
            run --release \
            -- \
            install \
            --skip-verify-and-downloads \
            --skip-kind from-archive \
            --skip-kind patched-from-archive \
            --skip-kind remapped-inline-file \
            --skip-kind inline-file \
            --skip-kind create-bsa

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
