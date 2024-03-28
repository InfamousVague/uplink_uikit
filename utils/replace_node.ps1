# Set the path to the mod.rs file
$modRsFilePath = ".\common\src\warp_runner\mod.rs"

# Read the contents of the mod.rs file
$modRsContent = Get-Content -Path $modRsFilePath -Raw

# Replace the old address with the new address in the mod.rs content
$newModRsContentFirst = $modRsContent -replace "/ip4/159.65.41.31/tcp/8848/", "/ip4/127.0.0.1/tcp/4444/"

# Set the new address based on the content of peerID.txt
$peerIDFilePath = ".\warp\peerID.txt"
$newPeerID = (Get-Content -Path $peerIDFilePath | Select-String -Pattern '/ip4/\d+\.\d+\.\d+\.\d+/tcp/\d+/p2p/(\S+)').Matches.Groups[1].Value

# Define the old address to be replaced
$oldPeerId = "12D3KooWRF2bz3KDRPvBs1FASRDRk7BfdYc1RUcfwKsz7UBEu7mL"

# Replace the old address with the new address in the mod.rs content
$newModRsContentTwo = $newModRsContentFirst -replace $oldPeerId, $newPeerID

# Write the modified content back to the mod.rs file
Set-Content -Path $modRsFilePath -Value $newModRsContentTwo

Write-Host "Replacement complete for Local Peer ID."
