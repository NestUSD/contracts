fn assert_authority(protocol: &Protocol, signer: &Signer) -> Result<()> {
    require_keys_eq!(protocol.authority, signer.key(), CoreError::Unauthorized);
    Ok(())
}
