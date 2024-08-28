using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Drawing;
using System.Globalization;
using System.Linq;
using System.Resources;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace Pacenotes_Installer
{
    public partial class LocalizationChangeForm : Form
    {
        public Dictionary<string, string> UIlanguages;
        public CultureInfo SelectedCulture { get; private set; }

        public LocalizationChangeForm(System.Globalization.CultureInfo defaultCulture)
        {
            InitializeComponent();

            // Load cultures from the installed .NET
            UIlanguages = new Dictionary<string, string>();
            UIlanguages["English"] = "en";
            UIlanguages["Polski"] = "pl";

            // Populate the ComboBox with cultures
            comboBoxCultures.Items.AddRange(UIlanguages.Keys.ToArray());

            comboBoxCultures.SelectedIndex = 0;

            // Select the default culture
            if (UIlanguages.ContainsValue(System.Globalization.CultureInfo.CurrentCulture.Parent.Name))
            {
                comboBoxCultures.SelectedIndex = UIlanguages.Values.ToList().IndexOf(System.Globalization.CultureInfo.CurrentCulture.Parent.Name);
            }

        }

        private void buttonOK_Click(object sender, EventArgs e)
        {
            if (comboBoxCultures.SelectedItem != null)
            {
                SelectedCulture = CultureInfo.GetCultureInfo(UIlanguages[comboBoxCultures.SelectedItem.ToString()]);
                this.DialogResult = DialogResult.OK;
                this.Close();
            }
        }
    }
}
