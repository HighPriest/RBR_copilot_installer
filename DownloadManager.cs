using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing.Text;
using System.IO.Compression;
using System.Linq;
using System.Security.Cryptography.X509Certificates;
using System.Security.Policy;
using System.Text;
using System.Threading.Tasks;
using Microsoft.IdentityModel.Tokens;
using Pacenotes_Installer.Classes;

namespace Pacenotes_Installer
{
    enum Status : byte
    {
        disconnected,
        connected,
        authorized,
        files_listed,
        downloaded
    }

    internal class DownloadManager
    {
        private static Supabase.Client supabase;
        private string publicURL, publicKey;
        public string privateKey;
        public Status status = 0;

        public List<Supabase.Storage.FileObject> public_files;
        public List<Supabase.Storage.FileObject> private_files;

        public DownloadManager()
        {
            string PublicURL = "";
            string PublicKey = "";

            var options = new Supabase.SupabaseOptions
            {
                AutoConnectRealtime = true
            };

            supabase = new Supabase.Client(publicURL, publicKey, options);
        }

        public void InitializeSupabase()
        {
            try
            {
                Task.Run(() => supabase.InitializeAsync()).Wait();
                status = Status.connected;
            }
            catch
            {
                status = Status.disconnected;
            }
        }

        public void InitializeSupabase(string key)
        {
            var options = new Supabase.SupabaseOptions
            {
                AutoConnectRealtime = true
            };

            privateKey = key;
            Supabase.Client _supabase = new Supabase.Client(publicURL, key, options);

            try
            {
                Task.Run(() => _supabase.InitializeAsync()).Wait();
                if (supabase.Realtime.Socket.IsConnected) supabase.Realtime.Disconnect();
                supabase = _supabase;
                status = Status.authorized;
            }
            catch
            {
                // Throw a box for the user, stating that their code was wrong
            }
        }

        #region InstallationMethods
        public void createRBRBackup(string directory)
        {

        }
        public void restoreRBRBackup(string directory)
        {

        }
        public void saveFile(string destinationDir, string fileName, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            if (fileName.EndsWith(".zip"))
            {
                ExtractZipFile(Task.Run(() => DownloadFile(fileName, backgroundWorker)).GetAwaiter().GetResult(), destinationDir);
            }
            else
            {
                try
                {
                    byte[] file = Task.Run(() => DownloadFile(fileName, backgroundWorker)).GetAwaiter().GetResult();
                    using (var fs = new FileStream(Path.Combine(destinationDir, fileName), FileMode.Create, FileAccess.Write))
                    {
                        fs.Write(file, 0, file.Length);
                        return;
                    }
                }
                catch (Exception ex)
                {
                    Console.WriteLine("Exception caught in process: {0}", ex);
                    return;
                }
            }
        }

        public void ExtractZipFile(byte[] zipFileBytes, string destinationDirectory)
        {
            // Convert byte[] to MemoryStream
            using (MemoryStream zipStream = new MemoryStream(zipFileBytes))
            {
                // Create a ZipArchive from the MemoryStream
                using (ZipArchive archive = new ZipArchive(zipStream))
                {
                    // Ensure the destination directory exists
                    Directory.CreateDirectory(destinationDirectory);

                    // Extract each entry to the destination directory
                    foreach (ZipArchiveEntry entry in archive.Entries)
                    {
                        string destinationPath = Path.Combine(destinationDirectory, entry.FullName);

                        // Create the directory if it doesn't exist
                        if (entry.FullName.EndsWith("/"))
                        {
                            Directory.CreateDirectory(destinationPath);
                        }
                        else
                        {
                            // Create the file and extract its contents
                            entry.ExtractToFile(destinationPath, overwrite: true);
                        }
                    }
                }
            }
        }
        #endregion InstallationMethods

        #region DownloadMethods
        public void ListAllFiles()
        {
            if (status == Status.disconnected || !supabase.Realtime.Socket.IsConnected)
            {
                return;
            }

            public_files = new List<Supabase.Storage.FileObject>();

            foreach (Supabase.Storage.FileObject _file in Task.Run(() => supabase.Storage.From("Public").List()).GetAwaiter().GetResult())
            {
                recursiveSupabaseFileListing(public_files, _file, "");
            }

            status = Status.files_listed;
            foreach (var file in public_files)
            {
                Debug.WriteLine("File Name: " + file.Name);
            }
            Debug.WriteLine("FilesListed: " + status);
        }

        public void recursiveSupabaseFileListing(List<Supabase.Storage.FileObject> files_list, Supabase.Storage.FileObject file, string path)
        {
            string _path = path.IsNullOrEmpty() ? file.Name : String.Join("/", path, file.Name);

            if (!file.IsFolder)
            {
                file.Name = _path;
                files_list.Add(file);
                return;
            }

            var files = Task.Run(() => supabase.Storage.From("Public").List(path: _path)).GetAwaiter().GetResult();

            Debug.WriteLine(_path);

            foreach (Supabase.Storage.FileObject _files in files)
            {
                recursiveSupabaseFileListing(files_list, _files, _path);
            }
        }

        public Task<byte[]> DownloadFile(string fileName, System.ComponentModel.BackgroundWorker backgroundWorker) 
        {
            // TODO: Check if supabase is initilized before proceeding
            var bytes = Task.Run(() => supabase.Storage
            .From("Public")
                        .Download(fileName, (sender, progress) => backgroundWorker.ReportProgress((int)progress, "Download: " + fileName)));
            return bytes;
        }
        #endregion DownloadMethods

        #region ConfigurationMethods
        public static string CreateRBRConfiguration(string destinationDir, ListView.CheckedListViewItemCollection coPilotConfiguration, ListView.CheckedListViewItemCollection styleConfiguration, ListView.CheckedListViewItemCollection languageConfiguration)
        {
            string[] configPacenote = [ "[SETTINGS]",
                                        "sounds = " + coPilotConfiguration[0].Tag.ToString().Split(".")[0],
                                        "language = " + languageConfiguration[0].Text,
                                        "replaySpeeds = 0.005 0.01 0.03 0.06 0.125 0.25 0.5 0.75 1 1.25 1.5 1.75 2 4 6 8 10",
                                        "enableGUI = 1",
                                        "showReplayHUD = 1",
                                        "muteReplayPacenotes = 0", ";"];
            string[] configStyle = [    ";",
                                        "; File:         Rbr.ini",
                                        "; Version:      1.0",
                                        "; Date:         2023-12-09",
                                        ";",
                                        "[PACKAGE::RBR]",
                                        "file0=packages\\"];

            switch (styleConfiguration[0].Text)
            {
                case "Numeric Standard":
                    configStyle[^1] = configStyle[^1] + ("FilipekMod_numeric_standard.ini");
                    break;
                case "Numeric 1to7":
                    configStyle[^1] = configStyle[^1] + "FilipekMod_numeric_1to7.ini";
                    break;
                case "Descriptive":
                    configStyle[^1] = configStyle[^1] + "FilipekMod_descriptive.ini";
                    break;
            }

            string[] configPaths = ["Plugins\\Pacenote\\config\\pacenotes\\RBR.ini",
                                "Plugins\\Pacenote\\config\\pacenotes\\Rbr-Enhanced.ini",
                                "Plugins\\Pacenote\\config\\pacenotes\\Numeric-fr.ini",
                                "Plugins\\Pacenote\\config\\pacenotes\\Numeric-cz.ini",
                                "Plugins\\Pacenote\\config\\pacenotes\\Descriptive-cz.ini",
                                "Plugins\\Pacenote\\config\\pacenotes\\Descriptive-fr.ini",
                                "Plugins\\Pacenote\\config\\pacenotes\\Descriptive-hu.ini"];

            File.WriteAllLines(Path.Combine(destinationDir, "Plugins\\Pacenote\\Pacenote.ini"), configPacenote);
            foreach (string configPath in configPaths)
            {
                File.WriteAllLines(Path.Combine(destinationDir, configPath), configStyle);
            }
            


            return @"CoDriver = " + coPilotConfiguration[0].Tag.ToString().Split(".")[0] + @"
GUI Language = " + languageConfiguration[0].Text + @"
Pacenotes style = " + styleConfiguration[0].Text;
        }

        public void backupRBRConfiguration(string directory, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            // Tag if a backup is needed. If all the files are already installed, then don't backup what already exists.
            bool backup = false; // CAUTION: May cause bugs, where important data doesn't get backed up.

            // Take the download location
            string sourceDir = Path.Combine(directory, "backup\\FilipekMod");
            // Take the backup file location
            string targetDir = Path.Combine(directory, "backup\\backup_Pacenote.zip");

            if (File.Exists(targetDir))
            {
                targetDir = targetDir.Replace("backup_Pacenote", "backup_Pacenote_" + $"{DateTime.Now:yy_MM_dd-HH_mm_ss}");
            }
            // Create ZipArchive in Memory Stream
            using (MemoryStream memoryStream = new MemoryStream())
            {
                using (ZipArchive archive = new ZipArchive(memoryStream, ZipArchiveMode.Create, true))
                {
                    var files = Directory.EnumerateFiles(sourceDir, "*", SearchOption.AllDirectories);
                    // Enumerate each file in sourceDirectory
                    foreach (var entry in files.Select((x, i) => new { Value = x, Index = i }))
                    {
                        // Get structured file information, to avoid parsing errors
                        FileInfo fileInfo = new FileInfo(
                            // Change directory to root for search for the original files
                            // For backup, we are interested in original files we are going to replace
                            entry.Value.Replace(sourceDir, directory)
                            );

                        // Create an entry inside the zipFile of the file we want to backup
                        ZipArchiveEntry zipFile = archive.CreateEntry(
                            Path.Combine(
                                // Important: directory in GetRelativePath, needs to be of root game directory
                                Path.GetRelativePath(directory, fileInfo.DirectoryName),
                                Path.GetFileName(fileInfo.FullName)
                                )
                            );
                        try
                        {
                            using (Stream entryStream = zipFile.Open())
                            using (StreamWriter streamWriter = new StreamWriter(entryStream))
                                {
                                    streamWriter.Write(entryStream);
                                }
                        }
                        catch
                        {
                            backup = true; // If at least one file doesn't exist, then we should save the backup of user files
                        }

                        backgroundWorker.ReportProgress(entry.Index * 100 / files.Count(), "Extract: " + fileInfo.FullName);
                    }
                }

                if ( backup )
                using (FileStream fileStream = new FileStream(targetDir, FileMode.Create))
                {
                    memoryStream.Position = 0;
                    memoryStream.WriteTo(fileStream);
                }
            }
        }

        #endregion ConfigurationMethods
    }
}
