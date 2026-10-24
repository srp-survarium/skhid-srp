package player_icons_fla
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
   import scaleform.gfx.Extensions;
   import survarium.Util;
   import survarium.controls.HUDIconLayer;
   import survarium.controls.HUDMarker;
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var iconControl:HUDIconLayer;
      
      public var movingLeft:Boolean;
      
      public var lastPosY:Number;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      // The key that hides the interface (exe patch 62, F11; docs/notes/hide-ui-key.md): the markers
      // and names over heads are this movie's root, hidden in one step with the hud; the movie runs on.
      public function hide_interface(hidden:Boolean) : void
      {
         visible = !hidden;
      }

      public function create_player_icon(id:uint, kind:uint = 0, baseScale:Number = 1) : void
      {
         this.iconControl.create_player_icon(id,kind,baseScale);
      }
      
      public function set_player_icon_name(id:uint, str:String) : void
      {
         this.iconControl.set_player_icon_name(id,str);
      }
      
      public function set_player_icon_type(id:uint, type:uint) : void
      {
         this.iconControl.set_player_icon_type(id,type);
      }
      
      public function set_player_icon_container(id:uint, container:Boolean) : void
      {
         this.iconControl.set_player_icon_container(id,container);
      }
      
      public function set_player_icon_label(id:uint, label:String) : void
      {
         this.iconControl.set_player_icon_label(id,label);
      }
      
      public function set_enemy_icon_ids(value:Array) : void
      {
         this.iconControl.set_enemy_icon_ids(value);
      }
      
      public function set_player_icon_team(id:uint, team:uint) : void
      {
         this.iconControl.set_player_icon_team(id,team);
      }
      
      public function set_player_team(team:*) : void
      {
         this.iconControl.set_player_team(team);
      }
      
      public function set_player_icons_props(value:Array) : void
      {
         trace("set_player_icons_props",value.length);
         var icons:uint = value.length;
         for(var i:uint = 0; i < icons; i++)
         {
            Util.logObject(value[i]);
            this.iconControl.set_player_icon_props(value[i]);
         }
         trace("propsSetted");
      }
      
      public function set_player_icon_pos(id:uint, posX:Number, posY:Number, scale:Number) : void
      {
         this.iconControl.set_player_icon_pos(id,posX,posY,scale);
      }
      
      public function set_player_icon_rotation(id:uint, rot:Number) : void
      {
         this.iconControl.set_player_icon_rotation(id,rot);
      }
      
      public function set_player_icon_visible(id:uint, bool:Boolean) : void
      {
         this.iconControl.set_player_icon_visible(id,bool);
      }
      
      public function set_player_name_visible(id:uint, bool:Boolean) : void
      {
         this.iconControl.set_player_name_visible(id,bool);
      }
      
      public function set_player_icon_glow(id:uint, bool:Boolean) : void
      {
         this.iconControl.set_player_icon_glow(id,true);
      }
      
      public function set_player_color(id:uint, red:uint, green:uint, blue:uint, alpha:uint = 255) : void
      {
         if(red == 255 && green == 0 && blue == 0)
         {
            this.set_player_icon_glow(id,true);
         }
         this.iconControl.set_player_color(id,red,green,blue,alpha);
      }
      
      public function set_player_icon_alpha(id:uint, alpha:uint) : *
      {
         this.iconControl.set_player_icon_alpha(id,alpha);
      }
      
      public function set_player_time_interval(value:uint) : void
      {
         this.iconControl.set_player_time_interval(value);
      }
      
      public function getRandomData(id:uint, name:String, x:int, y:int) : Object
      {
         return {
            id:id,
            name:name,
            x:x + Math.random() * 99,
            y:y + Math.random() * 99,
            z:0.2,
            visible:true,
            modifier_visible:false,
            name_visible:true,
            rgb:13434879,
            a:100
         };
      }
      
      public function updateProps() : void
      {
         var arr:Array = [];
         arr.push(this.getRandomData(1,"name_01",10,10));
         this.set_player_icons_props(arr);
      }
      
      public function onStageClick(e:Event) : void
      {
         this.updateProps();
      }
      
      internal function frame1() : *
      {
         iconControl = new HUDIconLayer();
         this.iconControl.register_icon_renderer(0,PlayerNameRenderer);
         this.iconControl.register_icon_renderer(1,MarkerRenderer);
         HUDMarker.setLabelRendererClass(MarkerLabel);
         addChildAt(this.iconControl,0);
         movingLeft = false;
         lastPosY = 0;
         if(Extensions.isGFxPlayer)
         {
            this.create_player_icon(2,0,0.5);
            this.create_player_icon(1,1,1);
            this.create_player_icon(3,1,1);
            this.set_player_icon_pos(2,125,125,2);
            this.set_player_icon_pos(3,125,225,2);
            this.set_player_icon_pos(1,50,125,1);
            this.set_player_icon_visible(1,true);
            this.set_player_icon_visible(3,true);
            this.set_player_icon_type(1,0);
            this.set_player_icon_type(3,0);
            this.set_player_color(1,255,0,0,255);
            this.set_player_icon_visible(2,true);
            this.set_player_icon_type(2,0);
            this.set_player_color(2,255,0,0,255);
            stage.addEventListener(MouseEvent.CLICK,this.onStageClick);
         }
      }
   }
}

