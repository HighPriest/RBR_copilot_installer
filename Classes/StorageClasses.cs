using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace Pacenotes_Installer.Classes
{
    using System.Collections.Generic;
    using System.Security.Cryptography.X509Certificates;

    #region RbrConfigurationsStorage
    internal class RbrConfigurationsStorage
    {
        public Style? Style = new();
        public Language? Language = new();
        public Voice? Voice = new();
    }

    public class Style
    {
        public string Name { get; set; }
        public string Path { get; set; }
        public Supabase.Storage.FileObject FileObject { get; set; }

        public override string ToString() => Name;
    }

    public class Language
    {
        public string Name { get; set; }
        public string Path { get; set; }
        public Supabase.Storage.FileObject FileObject { get; set; }

        public override string ToString() => Name;
    }

    public class Voice
    {
        public string Name { get; set; }
        public string Path { get; set; }
        public Supabase.Storage.FileObject FileObject { get; set; }

        public override string ToString() => Name;
    }
    #endregion RbrConfigurationsStorage

    #region CrewChiefConfigurationsStorage
    internal class CrewChiefConfigurationsStorage {
        // TODO: ToImplement
    }


    #endregion CrewChiefConfigurationsStorage

}
