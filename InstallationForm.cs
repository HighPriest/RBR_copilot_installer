using System.Diagnostics;
using System;
using System.Globalization;
using System.Windows.Forms;
using System.Xml.Linq;
using static System.Windows.Forms.DataFormats;
using System.ComponentModel;
using System.Drawing.Text;

namespace Pacenotes_Installer
{
    internal partial class InstallationForm : Form
    {
        private DownloadManager downloadManager;
        System.ComponentModel.ComponentResourceManager resources = new System.ComponentModel.ComponentResourceManager(typeof(InstallationForm));
        public InstallationForm(DownloadManager _downloadManager)
        {
            InitializeComponent();
            downloadManager = _downloadManager;
            InitializeTabUI(); // Hides tabs from the user
        }

        private void InitializeTabUI()
        {
            // Source: https://www.codeproject.com/Questions/614157/How-to-Hide-TabControl-Headers
            // Set the Appearance to FlatButtons
            tabControl1.Appearance = TabAppearance.FlatButtons;

            // Hide the tab headers
            tabControl1.ItemSize = new Size(0, 1);

            // Adjust the TabControl size
            tabControl1.Dock = DockStyle.Fill;

            // Select Sizing Mode
            tabControl1.SizeMode = TabSizeMode.Fixed;
        }

        private void Form1_Load(object sender, EventArgs e)
        {
            downloadManager.InitializeSupabase();
        }

        private void btn_back_Click(object sender, EventArgs e)
        {
            tabControl1.SelectTab(tabControl1.SelectedIndex - 1);
        }

        private void btn_next_Click(object sender, EventArgs e)
        {
            tabControl1.SelectTab(tabControl1.SelectedIndex + 1);
        }

        private void tab1buttonNext_Click(object sender, EventArgs e)
        {
            btn_next_Click(sender, e);
            tab2labelStatus.Text = downloadManager.status >= Status.connected ? "Online" : "Offline";
        }

        private void tab2buttonNext_Click(object sender, EventArgs e)
        {
            if (tab2contentKey.Text != downloadManager.privateKey)
            {
                downloadManager.InitializeSupabase(tab2contentKey.Text);
            }
            btn_next_Click(sender, e);
            if (downloadManager.status < Status.files_listed && downloadManager.status >= Status.connected)
            {
                // if correct code is passed, then the status gets reverted below flies_listed.
                // So, we don't have to worry that the user passes the screen without putting in code, then comes back to put the code in.
                workerListFiles.RunWorkerAsync();
            };
        }

        private void tab2buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        private void tab3buttonBrowse_Click(object sender, EventArgs e)
        {
            var btn = sender as Button;
            if (btn == null) { return; }

            FolderBrowserDialog fbd = new FolderBrowserDialog();
            fbd.AutoUpgradeEnabled = true;
            DialogResult result = fbd.ShowDialog();

            if (result == DialogResult.OK && !string.IsNullOrWhiteSpace(fbd.SelectedPath))
            {
                if (btn.Tag is "RSF_DIRECTORY")
                {
                    if (File.Exists(Path.Combine(fbd.SelectedPath, "RichardBurnsRally_SSE.exe")))
                        tab3dirRBR.Text = fbd.SelectedPath;
                    else
                        System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorRBRDirectory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                }
                if (btn.Tag is "CC_DIRECTORY")
                {
                    Debug.WriteLine(Path.Combine(fbd.SelectedPath, "CrewChiefV4.exe"));
                    if (File.Exists(Path.Combine(fbd.SelectedPath, "CrewChiefV4.exe")))
                        tab3dirCC.Text = fbd.SelectedPath;
                    else
                        System.Windows.Forms.MessageBox.Show(resources.GetString("ErrorCCDirectory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                }
            }
        }

        private void tab3buttonNext_Click(object sender, EventArgs e)
        {
            if (string.IsNullOrEmpty(tab3dirRBR.Text) && string.IsNullOrEmpty(tab3dirCC.Text))
            {
                System.Windows.Forms.MessageBox.Show(text: resources.GetString("ErrorNoDirectory"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Error);
                return;
            }
            btn_next_Click(sender, e);
        }

        private void tab3buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        private void tab4buttonNext_Click(object sender, EventArgs e)
        {
            // Check if each category has something selected
            foreach (TreeNode node in tab4treeView1.Nodes)
            {
                if (node.Checked) { continue; }
                else if (node.Nodes.Count > 0)
                {
                    if (!tab4AtLeastOneNodeChecked(node))
                    {
                        System.Windows.Forms.MessageBox.Show(text: resources.GetString("ErrorDownloadList"), caption: "ERROR", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Exclamation);
                        return;
                    };
                }
            }
            // Prepare Configuration tab for new content, based on what is being downloaded
            tab6selectCoPilot.Clear(); tab6selectLanguage.Clear(); tab6selectStyle.Clear();
            workerInstallation.RunWorkerAsync();
            btn_next_Click(sender, e);
        }

        private bool tab4AtLeastOneNodeChecked(TreeNode _node)
        {
            bool status = false;
            if (_node.Nodes.Count > 0)
            {
                foreach (TreeNode node in _node.Nodes)
                {
                    status = tab4AtLeastOneNodeChecked(node);
                    if (status) { break; }
                }
            }
            if (_node.Checked == true)
            {
                return true;
            }

            return status;

        }

        private void tab4buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        private void tab6buttonNext_Click(object sender, EventArgs e)
        {
            // Do saving tasks
            string config = DownloadManager.CreateRBRConfiguration(tab3dirRBR.Text,
                (Supabase.Storage.FileObject)tab6selectCoPilot.CheckedItems[0].Tag,
                (string)tab6selectStyle.CheckedItems[0].Text,
                (string)tab6selectLanguage.CheckedItems[0].Text);
            tab7textConfig.Text = config;
            btn_next_Click(sender, e);
        }

        private void tab6buttonBack_Click(object sender, EventArgs e)
        {
            // Do reset tasks
            tabControl1.SelectedIndex = 0;
        }

        private void tab6listViewHandler(object sender, EventArgs e)
        {
            BeginInvoke((MethodInvoker)delegate
            {
                ListView _sender = (ListView)sender;
                if (_sender.SelectedItems.Count > 0)
                {
                    foreach (ListViewItem item in _sender.CheckedItems)
                    {
                        item.Checked = false;
                    }
                    _sender.SelectedItems[0].Checked = true;
                    return;
                }
                try
                {
                    _sender.Items[0].Checked = true;
                }
                catch (Exception ex)
                {
                    Debug.Write(ex.ToString());
                }
            });

        }

        private void tab7buttonNext_Click(object sender, EventArgs e)
        {
            // Do closing tasks
            workerListFiles.Dispose();
            workerInstallation.Dispose();
            Close();
        }

        private void tab7buttonBack_Click(object sender, EventArgs e)
        {
            btn_back_Click(sender, e);
        }

        private void listAllFiles(object sender, System.ComponentModel.DoWorkEventArgs e)
        {
            downloadManager.ListAllFiles();
        }

        private void initializeFileList(object sender, System.ComponentModel.RunWorkerCompletedEventArgs e)
        {
            tab4treeView1.Nodes.Clear();
            TreeNode rootNode = new TreeNode("Root");

            foreach (var node in downloadManager.public_files)
            {
                addNodeToFileList(rootNode, node);
            }
            foreach (TreeNode node in rootNode.Nodes)
            {
                tab4treeView1.Nodes.Add(node);
            }

        }

        private void addNodeToFileList(TreeNode parentNode, Supabase.Storage.FileObject file)
        {
            List<string> segments = new List<string>(file.Name.Split("/"));
            segments[^1] = segments[^1].Replace(".zip", "").Replace(".ini", "");

            foreach (string segment in segments)
            {
                TreeNode childNode = parentNode.Nodes.ContainsKey(segment) ? parentNode.Nodes[segment] : null;

                if (childNode == null)
                {
                    // TreeNodes have 3 data slots: Text, Name, Tag!! Can't populate Name or Tag on creation!
                    // Tag: Supabase.Storage.FileObject
                    // ?? Name = Key. Maybe the extension is unnecessary, but the Key will be needed ?? Name: fileNameWithExtension
                    // Text: fileNameWithout Extension
                    childNode = new TreeNode(segment);
                    childNode.Name = segment;
                    if (segment == segments.Last())
                    {
                        // childNode.Name = pathSegment.Split("/").Last();
                        childNode.Tag = file;
                    }
                    parentNode.Nodes.Add(childNode);
                }
                parentNode.ExpandAll();
                parentNode = childNode;
            }
        }

        private void tab4treeView1_CheckChildNodes(object sender, TreeViewEventArgs e)
        {
            // Check if the node has children
            if (e.Node.Nodes.Count > 0)
            {
                // Set the checked state of all child nodes to match the parent node
                foreach (TreeNode childNode in e.Node.Nodes)
                {
                    childNode.Checked = e.Node.Checked;
                }
            }
        }

        private void workerInstallation_DoWork(object sender, System.ComponentModel.DoWorkEventArgs e)
        {
            // (Download) Downloads files & backups current configuration
            // Traverse all level 0 nodes and try to do download action on them
            foreach (TreeNode treeNode in tab4treeView1.Nodes)
            {
                downloadBasedOnTreeNode((TreeNode)treeNode);
            }

            // (Backup) Create a backup of files which we are going to replace
            // Note: This doesn't touch the files which are not going to be replaced
            downloadManager.backupRBRConfiguration(tab3dirRBR.Text, workerInstallation);

            // (Installation) Copy files to working directory, after download & backup
            downloadManager.installRBRConfiguration(tab3dirRBR.Text, workerInstallation);
            // Directory.Move(Path.Combine(tab3dirRBR.Text, "backup\\FilipekMod"), tab3dirRBR.Text);

            // Make sure at least one item is checked
            tab6listViewHandler(tab6selectLanguage, new EventArgs());
            tab6listViewHandler(tab6selectCoPilot, new EventArgs());
            tab6listViewHandler(tab6selectStyle, new EventArgs());
        }

        private void downloadBasedOnTreeNode(TreeNode treeNode)
        {
            if (treeNode.Checked & treeNode.Tag is Supabase.Storage.FileObject)
            {
                Supabase.Storage.FileObject file = (Supabase.Storage.FileObject)treeNode.Tag;
                string category = file.Name.Split("/").First();
                ListViewItem item = new ListViewItem(treeNode.Text);
                item.Name = treeNode.Name;
                item.Tag = treeNode.Tag;
                switch (category.ToLower())
                {
                    case "language":
                        tab6selectLanguage.Invoke(() => tab6selectLanguage.Items.Add(item));
                        break;
                    case "sounds":
                        tab6selectCoPilot.Invoke(() => tab6selectCoPilot.Items.Add(item));
                        break;
                    case "config":
                        tab6selectStyle.Invoke(() => tab6selectStyle.Items.Add(item));
                        break;
                }
                downloadManager.saveFile(Path.Combine(tab3dirRBR.Text, "backup\\FilipekMod"), (Supabase.Storage.FileObject)treeNode.Tag, workerInstallation);
                return;
            }
            foreach (TreeNode node in treeNode.Nodes)
            {
                downloadBasedOnTreeNode((TreeNode)node);
            }
        }

        private void workerInstallation_ProgressChanged(object sender, System.ComponentModel.ProgressChangedEventArgs e)
        {
            tab5progressBar1.Value = e.ProgressPercentage;
            tab5label1.Text = e.UserState.ToString();
        }

        private void workerInstallation_RunWorkerCompleted(object sender, System.ComponentModel.RunWorkerCompletedEventArgs e)
        {
            tabControl1.SelectTab(tabControl1.SelectedIndex + 1);
            System.Windows.Forms.MessageBox.Show(text: "Download completed succesfully!", caption: "SUCCESS", buttons: MessageBoxButtons.OK, icon: MessageBoxIcon.Asterisk);
        }
    }
}
