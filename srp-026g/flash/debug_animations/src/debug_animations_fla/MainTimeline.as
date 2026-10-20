package debug_animations_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.debug.Row;
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var names:Array;
      
      public var dataP:Array;
      
      public var _ofsetX:Number;
      
      public var _ofsetY:Number;
      
      public var _name_ofsetX:Number;
      
      public var rowH:int;
      
      public var _sx:int;
      
      public var _sy:int;
      
      public var columns:Vector.<Rower>;
      
      public var columnsNames:Array;
      
      public var columnsn:uint;
      
      public var content:MovieClip;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set vertival_offset(param1:Number) : *
      {
         _ofsetX = param1;
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function set horizontal_offset(param1:Number) : *
      {
         _ofsetY = param1;
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function getHeight() : int
      {
         return int(this.content.height);
      }
      
      public function setOffset(param1:*, param2:*) : *
      {
         var _loc3_:* = this._sx;
         _ofsetX = param1;
         _ofsetY = param2;
         var _loc4_:int = 0;
         var _loc5_:Row;
         var _loc6_:*;
         while(_loc4_ < this.columns.length)
         {
            _loc5_ = this.columns[_loc4_];
            _loc6_ = this.columnsNames[_loc4_];
            _loc5_.y = this._sy + 25;
            _loc5_.x = _loc3_;
            _loc5_.rowHeight = this.rowH + this._ofsetY;
            _loc5_.updateElemetns();
            _loc6_.y = this._sy + 5;
            _loc6_.x = _loc3_ + this._name_ofsetX;
            if(_loc5_.width)
            {
               _loc3_ += _loc5_.my_width + this._ofsetX;
            }
            _loc4_++;
         }
      }
      
      public function setShift(param1:int, param2:int = 0) : *
      {
         _sx = param1;
         _sy = param2;
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function setProps(param1:Array, param2:int = 0) : *
      {
         var _loc3_:* = this._sx;
         var _loc4_:int = 0;
         var _loc5_:TextFormat;
         var _loc6_:Rower;
         var _loc7_:TextField;
         while(_loc4_ < param1.length)
         {
            if(_loc4_ == this.columns.length || !this.columns[_loc4_])
            {
               _loc6_ = new Rower();
               _loc6_.mouseChildren = false;
               _loc6_.mouseEnabled = false;
               this.columns.push(_loc6_);
            }
            else
            {
               _loc6_ = this.columns[_loc4_];
            }
            _loc6_.rowHeight = this.rowH + this._ofsetY;
            _loc6_.my_width = param1[_loc4_].width;
            _loc6_.columns = 1;
            _loc5_ = new TextFormat();
            _loc5_.font = "tahoma";
            if(param1[_loc4_].textAlign == undefined)
            {
               param1[_loc4_].textAlign = 0;
            }
            if(param1[_loc4_].textAlign == 0)
            {
               _loc5_.align = "left";
            }
            else if(param1[_loc4_].textAlign == 1)
            {
               _loc5_.align = "center";
            }
            else
            {
               _loc5_.align = "right";
            }
            if(!this.columnsNames[_loc4_])
            {
               _loc7_ = new TextField();
               _loc7_.textColor = 16777215;
               this.columnsNames[_loc4_] = _loc7_;
               _loc7_.selectable = false;
               _loc7_.filters = [new DropShadowFilter(0,45,0,1,4,4,1.5)];
            }
            else
            {
               _loc7_ = this.columnsNames[_loc4_];
            }
            if(param1[_loc4_].name)
            {
               _loc7_.text = param1[_loc4_].name;
            }
            else
            {
               _loc7_.text = this.names[_loc4_];
            }
            _loc7_.width = param1[_loc4_].width - this._name_ofsetX;
            _loc7_.setTextFormat(_loc5_);
            _loc3_ += param1[_loc4_].width + this._ofsetX;
            this.content.addChild(_loc6_);
            this.content.addChild(_loc7_);
            _loc4_++;
         }
         while(_loc4_ < this.columns.length)
         {
            _loc6_ = this.columns[_loc4_];
            _loc7_ = this.columnsNames[_loc4_];
            this.content.removeChild(_loc6_);
            this.content.removeChild(_loc7_);
            _loc4_++;
         }
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function setData(param1:Array) : void
      {
         var _loc2_:int;
         var _loc3_:Array;
         var _loc4_:int;
         _loc2_ = 0;
         while(_loc2_ < this.columns.length)
         {
            this.columns[_loc2_].length = param1.length;
            _loc2_++;
         }
         _loc2_ = 0;
         while(_loc2_ < param1.length)
         {
            _loc3_ = param1[_loc2_];
            _loc4_ = 0;
            while(_loc4_ < _loc3_.length)
            {
               this.columns[_loc4_].elements[_loc2_].setData(param1[_loc2_][_loc4_]);
               _loc4_++;
            }
            _loc2_++;
         }
      }
      
      public function onStageResize(param1:Event = null) : *
      {
         this.content.x = 0;
         this.content.y = 0;
      }
      
      internal function frame1() : *
      {
         names = ["Object name","Synchronization group\t","Additivity layer","Bones mask","Animation name","Weight","Time","scale","Time","Interval id","Interval time"];
         stage.scaleMode = StageScaleMode.NO_SCALE;
         dataP = [{
            "width":80,
            "textAlign":1
         },{"width":100},{"width":50},{"width":20},{"width":20},{"width":100},{"width":100},{"width":40},{"width":25},{"width":80}];
         _ofsetX = 2;
         _ofsetY = 2;
         _name_ofsetX = 2;
         rowH = 20;
         _sx = 10;
         _sy = 10;
         columns = new Vector.<Rower>();
         columnsNames = new Array();
         columnsn = 5;
         content = new MovieClip();
         addChild(this.content);
         this.setProps(this.dataP);
         dataP = [{
            "width":20,
            "textAlign":1
         },{"width":40},{"width":40},{"width":20},{"width":20},{"width":100},{"width":100},{"width":40},{"width":25},{"width":80}];
         this.setProps(this.dataP);
         stage.scaleMode = "noScale";
         stage.align = "TL";
         stage.addEventListener(Event.RESIZE,this.onStageResize);
      }
   }
}

