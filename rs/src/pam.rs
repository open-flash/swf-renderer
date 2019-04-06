pub fn write_pam<W>(
  writer: &mut W,
  width: usize,
  height: usize,
  image_data: &[u8],
) -> ::std::io::Result<()> where W: ::std::io::Write {
  assert_eq!(image_data.len(), width * height * 4);

  writer.write_all(b"P7\n")?;

  writer.write_all(b"WIDTH ")?;
  writer.write_all(format!("{}", width).as_bytes())?;
  writer.write_all(b"\n")?;

  writer.write_all(b"HEIGHT ")?;
  writer.write_all(format!("{}", height).as_bytes())?;
  writer.write_all(b"\n")?;

  writer.write_all(b"DEPTH 4\n")?;
  writer.write_all(b"MAXVAL 255\n")?;
  writer.write_all(b"TUPLTYPE RGB_ALPHA\n")?;
  writer.write_all(b"ENDHDR\n")?;
  writer.write_all(&image_data)?;

  Ok(())
}
