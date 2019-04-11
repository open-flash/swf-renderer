use crate::renderer::Image;

pub fn write_pam<W>(
  writer: &mut W,
  image: &Image,
) -> ::std::io::Result<()> where W: ::std::io::Write {
  let bytes_per_pixel: usize = 4;
  let bytes_per_row: usize = image.meta.width * bytes_per_pixel;

  debug_assert!(image.meta.stride >= bytes_per_row);
  debug_assert_eq!(image.data.len(), image.meta.height * image.meta.stride);

  writer.write_all(b"P7\n")?;

  writer.write_all(b"WIDTH ")?;
  writer.write_all(format!("{}", image.meta.width).as_bytes())?;
  writer.write_all(b"\n")?;

  writer.write_all(b"HEIGHT ")?;
  writer.write_all(format!("{}", image.meta.height).as_bytes())?;
  writer.write_all(b"\n")?;

  writer.write_all(b"DEPTH 4\n")?;
  writer.write_all(b"MAXVAL 255\n")?;
  writer.write_all(b"TUPLTYPE RGB_ALPHA\n")?;
  writer.write_all(b"ENDHDR\n")?;
  for y in 0..image.meta.height {
    let start = y * image.meta.stride;
    let end = start + bytes_per_row;
    writer.write_all(&image.data[start..end])?;
  }

  Ok(())
}
