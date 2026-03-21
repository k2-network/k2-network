#!/bin/bash
WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 \
GIO_USE_TLS=openssl \
LD_PRELOAD=/lib/x86_64-linux-gnu/libpthread.so.0 \
npm run tauri dev
