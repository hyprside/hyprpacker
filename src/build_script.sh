# install all packages inside /deps/ with pacman
mkdir /deps -p
buildDependencies=$(find /deps/ -type f -name "*.pkg.tar.zst")
pacman -y
if [ -n "$buildDependencies" ]; then
	pacman -U --needed --noconfirm $buildDependencies
fi
pacman -S --needed --noconfirm sudo # Install sudo
useradd builduser -m # Create the builduser
passwd -d builduser # Delete the buildusers password
printf 'builduser ALL=(ALL) ALL\nDefaults    env_keep += "PKGDEST"\nDefaults    env_keep += "BUILDDIR"\n' | tee -a /etc/sudoers # Allow the builduser passwordless sudo
cd /src
rm -rf /out/makepkg/pkg
rm -rf /out/makepkg/*.pkg.tar.zst
rm -rf /out/*.pkg.tar.zst
mkdir /out/makepkg -p
chown builduser:builduser /out/ -R
sudo -u builduser bash -c 'makepkg --noconfirm --noprogressbar -s -C -f'
