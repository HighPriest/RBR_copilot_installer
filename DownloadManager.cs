using System.Diagnostics;
using System.IO.Compression;
using Microsoft.IdentityModel.Tokens;

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
        private Supabase.Client supabase;
        private string publicURL, publicKey;
        public string privateKey;
        public Status status = 0;

        public List<Supabase.Storage.FileObject> public_files;
        public List<Supabase.Storage.FileObject> private_files;

        public DownloadManager()
        {
            if(Environment.GetEnvironmentVariable("SUPABASE_PUBLICURL").IsNullOrEmpty() || Environment.GetEnvironmentVariable("SUPABASE_PUBLICKEY").IsNullOrEmpty())
            {
                publicURL = Properties.Secrets.publicUrl;
                publicKey = Properties.Secrets.publicKey;
            } else
            {
                publicURL = Environment.GetEnvironmentVariable("SUPABASE_PUBLICURL");
                publicKey = Environment.GetEnvironmentVariable("SUPABASE_PUBLICKEY");
            }

            var options = new Supabase.SupabaseOptions
            {
                //AutoConnectRealtime = true
            };

            supabase = new Supabase.Client(publicURL, publicKey, options);
        }

        public void InitializeSupabase()
        {
            try
            {
                Task.Run(() => supabase.InitializeAsync()).Wait();
                status = Status.connected;
                //Debug.WriteLine(supabase.Realtime.Socket.IsConnected);
            }
            catch
            {
                status = Status.disconnected;
            }
        }

        public void InitializeSupabase(string privateKey)
        {
            this.privateKey = privateKey;
            var options = new Supabase.SupabaseOptions
            {
                //AutoConnectRealtime = true,
                Headers = new Dictionary<string, string>(){
                    { "privateKey", privateKey } // Do get Premium content
                }
            };

            Supabase.Client _supabase = new Supabase.Client(publicURL, publicKey, options);

            try
            {
                Task.Run(() => _supabase.InitializeAsync()).Wait();
                //if (supabase.Realtime.Socket.IsConnected) supabase.Realtime.Disconnect();
                supabase = _supabase;
                status = Status.authorized;
            }
            catch
            {
                // TODO: Throw a box for the user, stating that their code was wrong
            }
        }

        #region InstallationMethods
        public void createRBRBackup(string directory)
        {

        }
        public void restoreRBRBackup(string directory)
        {

        }
        public void saveFile(string destinationDir, Supabase.Storage.FileObject file, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            // Check if file has already been downloaded & skip the download phase if it exists
            if (File.Exists(Path.Combine(destinationDir, file.Name)) && File.GetLastWriteTime(Path.Combine(destinationDir, file.Name)) > file.UpdatedAt)
            {
                return;
            }

            try
            {
                byte[] fileBytes = Task.Run(() => DownloadFile(file, backgroundWorker)).GetAwaiter().GetResult();
                Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(destinationDir,file.Name)));
                using (var fs = new FileStream(Path.Combine(destinationDir, file.Name), FileMode.Create, FileAccess.Write))
                {
                    fs.Write(fileBytes, 0, fileBytes.Length);
                    return;
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine("Exception caught in process: {0}", ex);
                return;
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
            if (status == Status.disconnected)
            {
                return;
            }

            public_files = new List<Supabase.Storage.FileObject>();

            foreach (Supabase.Storage.Bucket _bucket in Task.Run(() => supabase.Storage.ListBuckets()).GetAwaiter().GetResult())
            {
                foreach (Supabase.Storage.FileObject _file in Task.Run(() => supabase.Storage.From(_bucket.Id).List()).GetAwaiter().GetResult())
                {
                    _file.BucketId = _bucket.Id; // This is really stupid. Why isn't the bucketId filled on fetch?
                    recursiveSupabaseFileListing(public_files, _file, "");
                }
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

            var files = Task.Run(() => supabase.Storage.From(file.BucketId).List(path: _path)).GetAwaiter().GetResult();

            Debug.WriteLine(_path);

            foreach (Supabase.Storage.FileObject _file in files)
            {
                _file.BucketId = file.BucketId; // This is really stupid. Why isn't the bucketId filled on fetch?
                recursiveSupabaseFileListing(files_list, _file, _path);
            }
        }

        public Task<byte[]> DownloadFile(Supabase.Storage.FileObject file, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            // TODO: Check if supabase is initialized before proceeding
            var bytes = Task.Run(() => supabase.Storage
            .From(file.BucketId)
                        .Download(file.Name, (sender, progress) => backgroundWorker.ReportProgress((int)progress, "Download: " + file.Name)));
            return bytes;
        }
        #endregion DownloadMethods

        #region RBRConfigurationMethods
        public static string CreateRBRConfiguration(string destinationDir, string coPilotConfiguration, string styleConfiguration, string languageConfiguration)
        {

            string[] configPacenote = [
                                        ";",
                                        "; File:         PaceNote.ini",
                                        "; Version:      2.0",
                                        "; Date:         " + DateTime.Now.ToString("yyyy-MM-dd"),
                                        ";",
                                        "[SETTINGS]",
                                        "sounds = " + coPilotConfiguration,
                                        "language = " + languageConfiguration,
                                        "replaySpeeds = 0.005 0.01 0.03 0.06 0.125 0.25 0.5 0.75 1 1.25 1.5 1.75 2 4 6 8 10",
                                        "enableGUI = 1",
                                        "showReplayHUD = 1",
                                        "muteReplayPacenotes = 0", ";"];
            string[] configStyle = [    ";",
                                        "; File:         Rbr.ini",
                                        "; Version:      2.0",
                                        "; Date:         " + DateTime.Now.ToString("yyyy-MM-dd"),
                                        ";",
                                        "[PACKAGE::RBR]",
                                        "file0=packages\\"];

            switch (styleConfiguration)
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

            File.WriteAllLines(Path.Combine(destinationDir, "Plugins\\Pacenote\\PaceNote.ini"), configPacenote);
            foreach (string configPath in configPaths)
            {
                // Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(destinationDir, configPath))); // This is not necessary. If the config dir does not exist, plugin needs to be installed
                try
                {
                    File.WriteAllLines(Path.Combine(destinationDir, configPath), configStyle);
                } catch (Exception ex) {
                    System.Windows.Forms.MessageBox.Show(text: "The plugin configuration directory doesn't exist!\nThere is a high chance the Pacenote plugin is broken. First install the Pacenote plugin from RSF installer.", caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                }
            }



            return @"CoDriver = " + coPilotConfiguration + @"
GUI Language = " + languageConfiguration + @"
Pacenotes style = " + styleConfiguration;
        }

        public void backupRBRConfiguration(string directory, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            // Tag if a backup is needed. If all the files are already installed, then don't backup what already exists.
            bool backup = false; // CAUTION: May cause bugs, where important data doesn't get backed up.

            // Take the unpacked location
            string sourceDir = Path.Combine(directory, "backup\\temp");
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
                        if (fileInfo.Exists)
                        {
                            // Create an entry inside the zipFile of the file we want to backup
                            ZipArchiveEntry zipFile = archive.CreateEntry(
                                Path.Combine(
                                    // Important: directory in GetRelativePath, needs to be of root game directory
                                    Path.GetRelativePath(directory, fileInfo.DirectoryName),
                                    Path.GetFileName(fileInfo.FullName)
                                    )
                                );
                            using (Stream entryStream = zipFile.Open())
                            using (StreamWriter streamWriter = new StreamWriter(entryStream))
                            {
                                streamWriter.Write(entryStream);
                            }
                        }
                        else
                        {
                            backgroundWorker.ReportProgress(0, "Writing Backup");
                            backup = true; // If at least one file doesn't exist, then we should save the backup of user files
                        }
                    }
                }

                if (backup)
                    using (FileStream fileStream = new FileStream(targetDir, FileMode.Create))
                    {
                        backgroundWorker.ReportProgress(70, "Writing Backup");
                        memoryStream.Position = 0;
                        memoryStream.WriteTo(fileStream);
                        backgroundWorker.ReportProgress(100, "Backup Complete");
                        Thread.Sleep(1000); // Show off that something happened
                    }
            }
        }

        public void unpackRBRConfiguration(string targetDir, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            // Take the download location
            string sourceDir = Path.Combine(targetDir, "backup\\FilipekMod");

            // Take the unpacking location
            string unpackDir = Path.Combine(targetDir, "backup\\temp");

            // for each file in directory
            // check if the file is selected in configuration window
            // tab6selectCoPilot.CheckedItems[0].Text;
            // tab6selectStyle.CheckedItems[0].Text;
            // tab6selectLanguage.CheckedItems[0].Text;
            // Defer these items to the end of the list

            var files = Directory.EnumerateFiles(sourceDir, "*", SearchOption.AllDirectories);
            int files_count = files.Count();
            foreach (var file in files.Select((x,i) => new { Value = x, Index = i}))
            {
                if (file.Value.EndsWith(".zip"))
                {
                    ExtractZipFile(System.IO.File.ReadAllBytes(file.Value), unpackDir);
                }
                else
                {
                    string _unpackDir = file.Value.Replace("backup\\FilipekMod\\", "backup\\temp\\");
                    Directory.CreateDirectory(Path.GetDirectoryName(_unpackDir));
                    System.IO.File.Move(file.Value, _unpackDir, true);
                    if (((file.Index * 100 / files_count) % 10) == 0)
                    {
                        backgroundWorker.ReportProgress((file.Index * 100 / files_count) % 100, "Copying: " + file.Value);
                    }
                }
            }
        }
        public void installRBRConfiguration(string targetDir, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            string unpackDir = Path.Combine(targetDir, "backup\\temp");

            var files = Directory.EnumerateFiles(unpackDir, "*", SearchOption.AllDirectories);
            int files_count = files.Count();

            foreach (var file in files.Select((x, i) => new { Value = x, Index = i }))
            {
                targetDir = file.Value.Replace("backup\\temp\\", "");
                Directory.CreateDirectory(Path.GetDirectoryName(targetDir));
                System.IO.File.Move(file.Value, targetDir, true);
                if (((file.Index * 100 / files_count) % 10) == 0)
                {
                    backgroundWorker.ReportProgress((file.Index * 100 / files_count) % 100, "Installing: " + targetDir);
                }
            }
        }

        public void installRBRPackage(string targetDir, string packageName, System.ComponentModel.BackgroundWorker backgroundWorker)
        {
            // This can't be executed in WorkerComplete! The background worker is attached here, but can't be used.
            // backgroundWorker.ReportProgress(0, "Configuring: " + packageName);
            string packagePath = Path.Combine(targetDir, "backup\\FilipekMod\\", packageName);
            Directory.CreateDirectory(Path.GetDirectoryName(targetDir));
            if (packagePath.EndsWith(".zip"))
            {
                ExtractZipFile(System.IO.File.ReadAllBytes(packagePath), targetDir);
            }

        }
        #endregion RBRConfigurationMethods
    }
}
