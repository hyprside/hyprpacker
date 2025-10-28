FROM archlinux:base-devel

RUN pacman -Syu --noconfirm \
    bc \
    bison \
    cpio \
    flex \
    git \
    inetutils \
    kmod \
    libelf \
    llvm \
    openssl \
    pahole \
    perl \
    python \
    rsync \
    tar \
    which \
    xz \
    zstd \
    && pacman -Scc --noconfirm

ENV LANG=C.UTF-8
WORKDIR /work
