package survarium.controls
{
   import flash.events.MouseEvent;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public class TileListFosused extends TileList
   {
      
      public var overIndex:int;
      
      protected var _lastIndex:int = -1;
      
      public function TileListFosused()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         addEventListener(MouseEvent.MOUSE_OVER,this.newIndex);
         addEventListener(MouseEvent.MOUSE_OUT,this.newIndex);
      }
      
      protected function newIndex(param1:MouseEvent) : void
      {
         if(param1.target is IListItemRenderer)
         {
            overIndex = param1.target.index;
         }
         if(param1.type == MouseEvent.MOUSE_OUT)
         {
            overIndex = -1;
         }
      }
      
      override protected function changeFocus() : void
      {
         var _loc1_:Boolean = false;
         var _loc2_:IListItemRenderer;
         var _loc3_:IListItemRenderer;
         if(focused > 0 && this.overIndex != -1)
         {
            _loc3_ = getRendererAt(this.overIndex,_scrollPosition);
            _selectedIndex = this.overIndex;
            if(this._lastIndex != -1 && this.overIndex != this._lastIndex)
            {
               _loc2_ = getRendererAt(this._lastIndex,_scrollPosition);
            }
         }
         else
         {
            _loc3_ = getRendererAt(_selectedIndex,_scrollPosition);
         }
         if(_loc2_ != null)
         {
            _loc2_.selected = false;
            _loc2_.displayFocus = false;
            _loc2_.validateNow();
         }
         if(_loc3_ != null)
         {
            if(focused > 0 && !_loc1_)
            {
               _loc3_.displayFocus = true;
               _loc3_.selected = true;
               _loc3_.validateNow();
            }
            else
            {
               if(!_loc1_)
               {
                  _lastIndex = _loc3_.index;
               }
               _loc3_.selected = false;
               _loc3_.displayFocus = false;
               _loc3_.validateNow();
            }
         }
      }
   }
}

