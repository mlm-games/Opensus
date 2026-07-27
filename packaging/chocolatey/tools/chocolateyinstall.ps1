$ErrorActionPreference = 'Stop'
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$packageArgs = @{
    packageName   = 'my-ecosystem-bevy'
    fileType      = 'exe'
    url           = 'https://github.com/mlm-games/my-ecosystem-bevy/releases/latest'
    softwareName  = 'my-ecosystem-bevy'
    checksum      = ''
    checksumType  = 'sha256'
}
Install-ChocolateyPackage @packageArgs
