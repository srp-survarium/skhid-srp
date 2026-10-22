package survarium.constants
{
   // Which ammo the ammo panel shows (AmmoShareLite.activeIndex; AmmoShareControll.reflow's index):
   // the paper doll's selected weapon (LocalPaperDoll.selectedIndex) or every element.
   public class AmmoView
   {
      
      // reflow: take the paper doll's selected weapon
      public static const USE_PAPERDOLL_SELECTION:int = -2;
      
      public static const PRIMARY:int = 0;
      
      public static const SECONDARY:int = 1;
      
      // meaning by use: reflowElements lays out every element for it (also the third quick slot's index)
      public static const ALL:int = 4;
      
      // the panel's ammo slots of one weapon
      public static const SLOTS_PER_WEAPON:int = 3;
      
      // of both weapons
      public static const SLOTS:int = 6;
      
      public function AmmoView()
      {
         super();
      }
   }
}
