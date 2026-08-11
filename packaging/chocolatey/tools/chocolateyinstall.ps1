$ErrorActionPreference = 'Stop'
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$packageArgs = @{
    packageName   = 'opensus'
    fileType      = 'exe'
    url           = 'https://github.com/mlm-games/opensus/releases/latest'
    softwareName  = 'opensus'
    checksum      = ''
    checksumType  = 'sha256'
}
Install-ChocolateyPackage @packageArgs
