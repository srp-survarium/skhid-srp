package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Sprite;
   import flash.geom.Matrix;
   import flash.utils.Dictionary;

   // What a hidden interface still shows (Blackout's forced interface, exe patch 65; docs/notes/
   // blackout.md): a layer on the stage over the interface's root, placed as the root is (place), so
   // hiding the root leaves what is in it. A part kept here keeps its place on the screen and works
   // as in the root: the layer itself takes no mouse, its parts do (the kits are clicked here).
   public class KeptLayer extends Sprite
   {
      private var _root:DisplayObjectContainer;

      // called with a part after it moved, either way (the hud's layout goes on placing its parts)
      private var _moved:Function;

      private const _keptAt:Dictionary = new Dictionary();

      public function KeptLayer(root:DisplayObjectContainer, moved:Function = null)
      {
         super();
         this._root = root;
         this._moved = moved;
         mouseEnabled = false;
      }

      // Where the root is now (it may have been resized since).
      public function place() : void
      {
         transform.matrix = this._root.transform.matrix;
      }

      // Every part kept now.
      public function kept() : Array
      {
         var parts:Array = [];
         for(var part:Object in this._keptAt)
         {
            parts.push(part);
         }
         return parts;
      }

      // `parts` into the layer or back to where each was. In the layer a part's matrix is its own
      // times its parents' up to the root; it remembers its parent, its index there and its own
      // matrix to go back.
      public function keep(parts:Array, kept:Boolean) : void
      {
         this.place();
         for each(var part:DisplayObject in parts)
         {
            if(part == null)
            {
               continue;
            }
            var was:Object = this._keptAt[part];
            if(kept && was == null && part.parent != null)
            {
               var place:Matrix = part.transform.matrix.clone();
               for(var up:DisplayObjectContainer = part.parent; up != null && up != this._root; up = up.parent)
               {
                  place.concat(up.transform.matrix);
               }
               this._keptAt[part] = {
                  "parent":part.parent,
                  "index":part.parent.getChildIndex(part),
                  "matrix":part.transform.matrix.clone()
               };
               addChild(part);
               part.transform.matrix = place;
               this.moved(part);
            }
            else if(!kept && was != null)
            {
               var parent:DisplayObjectContainer = was.parent as DisplayObjectContainer;
               parent.addChildAt(part,Math.min(int(was.index),parent.numChildren));
               part.transform.matrix = was.matrix as Matrix;
               delete this._keptAt[part];
               this.moved(part);
            }
         }
      }

      private function moved(part:DisplayObject) : void
      {
         if(this._moved != null)
         {
            this._moved(part);
         }
      }
   }
}
