package survarium.upgrade
{
   // What a weapon's kit holds of a ModuleTree's kinds, as the Attachments window shows it: its scope
   // and the way it is on (ScopeMount), its muzzle device and its grip, each the inventory's item or
   // null.
   public class Fitted
   {

      public var scope:Object;

      public var way:int;

      public var muzzle:Object;

      public var grip:Object;

      public function Fitted(scope:Object, way:int, muzzle:Object, grip:Object)
      {
         super();
         this.scope = scope;
         this.way = way;
         this.muzzle = muzzle;
         this.grip = grip;
      }

      // The item of a kind the tree's slots take (ModuleTree.MUZZLE, GRIP, SCOPE).
      public function item(takes:String) : Object
      {
         return takes == ModuleTree.MUZZLE ? this.muzzle : (takes == ModuleTree.GRIP ? this.grip : this.scope);
      }
   }
}
