package survarium.controls
{
   import flash.display.BitmapData;
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.data.DataProvider;
   
   public class ContextList extends MovieClip
   {
      
      public var list:ButtonBar;
      
      public var contextBack:Sprite;
      
      public var selectedBack:Sprite;
      
      public var contextBackOver:Sprite;
      
      private var _rendererHeight:uint = 24;
      
      private var _pattern:BitmapData;
      
      private var spr:Sprite = new Sprite();
      
      private var border:uint = 10;
      
      private var newHeight:Number = 0;
      
      public function ContextList()
      {
         super();
         trace("ContextList",this.list,this.spr,this.contextBack);
         this.list.addEventListener(MouseEvent.MOUSE_OVER,this.onButtonMouseOver);
         this.list.addEventListener(MouseEvent.MOUSE_OUT,this.onButtonMouseOut);
         addChildAt(this.spr,2);
         this.selectedBack.visible = false;
         this.list.addEventListener(Event.RESIZE,this.updateSize);
      }
      
      private function onButtonMouseOut(param1:MouseEvent) : void
      {
         this.selectedBack.visible = false;
      }
      
      private function onButtonMouseOver(param1:MouseEvent) : void
      {
         if(param1.target === this.list)
         {
            return;
         }
         this.selectedBack.visible = true;
         this.selectedBack.y = this.list.y + param1.target.y;
         this.selectedBack.x = this.contextBack.x + 1;
      }
      
      public function setPatternt(param1:BitmapData) : void
      {
         if(this._pattern == param1)
         {
            return;
         }
         _pattern = param1;
      }
      
      protected function updateSize(param1:Event = null) : void
      {
         var _loc2_:ButtonBar = this.list;
         var _loc3_:Sprite = this.contextBack;
         var _loc4_:Sprite = this.contextBackOver;
         var _loc5_:int = _loc2_.getMaxWidth();
         var _loc6_:Graphics;
         _loc2_.width = _loc5_ + 10;
         _loc3_.height = this.newHeight + this.border * 2;
         _loc3_.width = _loc5_;
         _loc4_.x = 3;
         _loc4_.y = 2;
         _loc4_.height = _loc3_.height - 8;
         _loc4_.width = _loc3_.width - 6;
         this.selectedBack.width = this.contextBack.width - 2;
         if(this._pattern)
         {
            this.spr.alpha = 0.2;
            this.spr.x = _loc3_.x;
            this.spr.y = _loc3_.y;
            _loc6_ = this.spr.graphics;
            _loc6_.clear();
            _loc6_.beginBitmapFill(this._pattern,null,true,true);
            _loc6_.drawRect(4,4,_loc3_.width - 8,_loc3_.height - 12);
            _loc6_.endFill();
         }
      }
      
      public function set data(param1:Object) : void
      {
         var _loc2_:ButtonBar;
         _loc2_ = this.list;
         var _loc3_:Sprite = this.contextBack;
         var _loc4_:Sprite = this.contextBackOver;
         _loc2_.selectedIndex = -1;
         _loc2_.autoResize = true;
         var _loc5_:* = param1;
         _loc2_.dataProvider = new DataProvider(_loc5_);
         _loc2_.height = _loc5_.length * this._rendererHeight;
         newHeight = _loc5_.length * this._rendererHeight;
         _loc2_.validateNow();
         _loc2_.x = 2;
         _loc2_.y = this.border;
         this.updateSize();
      }
   }
}

