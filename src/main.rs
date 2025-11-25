mod bpb;

use std::env;
use std::fs::File;
use std::process;
use bpb::BiosParameterBlock;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <image_fat32>", args[0]);
        process::exit(1);
    }
    let filename = &args[1];

    println!("Ouverture de l'image : {}", filename);

    // 2. Ouvrir le fichier
    let mut file = match File::open(filename) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Erreur lors de l'ouverture du fichier: {}", e);
            process::exit(1);
        }
    };

    // 3. Lire le BPB
    match BiosParameterBlock::new(&mut file) {
        Ok(bpb) => {
            println!("--- Informations BPB ---");
            println!("Bytes par secteur   : {}", bpb.bytes_per_sector);
            println!("Secteurs par cluster: {}", bpb.sectors_per_cluster);
            println!("Secteurs réservés   : {}", bpb.reserved_sectors);
            println!("Nombre de FATs      : {}", bpb.num_fats);
            println!("Taille FAT32 (sect) : {}", bpb.fat_size_32);
            println!("Cluster Racine      : {}", bpb.root_cluster);
            println!("Signature           : OK");
        },
        Err(e) => {
            eprintln!("Erreur lors de la lecture du BPB: {}", e);
            process::exit(1);
        }
    }
}