#!/usr/bin/env bash

if [ -z "$1" ]; then
    echo "Usage: $0 <file>"
    exit 1
else
    echo "File: $1"
    echo "Content:"
    cat $1
fi